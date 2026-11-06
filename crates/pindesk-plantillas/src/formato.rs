//! **El fichero `.pdplantilla`** y **la reparacion**.
//!
//! El fichero es JSON con `"formato": "pindesk-plantilla"` y su `version`
//! (ver `docs/plantillas.md`). Se lee siempre por [`reparar`]: lo que venga
//! de una version vieja, de otro programa o de un modelo de IA pasa por las
//! mismas reglas. Se aceptan los nombres en ingles que suelen escribir los
//! modelos (`type`, `children`, `row`, `checklist`…), los numeros escritos
//! como texto, y se acota todo (filas, horas, profundidad, cuantos bloques):
//! lo que no se entiende se quita y se apunta en los avisos, nunca rompe la
//! lectura.

use serde_json::{Map, Value};

use crate::modelo::{
    Bloque, Cada, Documento, HUECO, Pagina, Repeticion, Tam, Tamano, Tipo, VERSION_FORMATO,
};
use crate::tema::{Color, Tema};

/// La extension de los ficheros.
pub const EXTENSION: &str = "pdplantilla";
/// Lo que dice el fichero que es.
pub const MARCA: &str = "pindesk-plantilla";
/// Lo mas hondo que puede ser el arbol.
pub const MAX_HONDO: usize = 10;
/// Cuantos bloques como mucho.
pub const MAX_BLOQUES: usize = 400;
/// Lo mas largo de un texto.
pub const MAX_TEXTO: usize = 2000;
/// Cuantos elementos como mucho en una lista.
pub const MAX_ITEMS: usize = 60;

/// Por que no se pudo leer.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorFormato {
    /// No es JSON (ni lleva JSON dentro).
    NoEsJson(String),
    /// De una version mas nueva de la app.
    MasNueva(u32),
    /// No hay ningun bloque que se entienda.
    Vacio,
}

impl std::fmt::Display for ErrorFormato {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorFormato::NoEsJson(e) => write!(f, "no es JSON: {e}"),
            ErrorFormato::MasNueva(v) => write!(f, "formato {v}, mas nuevo que {VERSION_FORMATO}"),
            ErrorFormato::Vacio => write!(f, "sin bloques"),
        }
    }
}

/// **Escribe** un documento.
pub fn escribir(doc: &Documento) -> String {
    let mut v = serde_json::to_value(doc).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        // La marca primero, para quien abra el fichero con un editor.
        let mut con = Map::new();
        con.insert("formato".into(), Value::String(MARCA.into()));
        con.insert("version".into(), Value::from(VERSION_FORMATO));
        for (k, x) in std::mem::take(m) {
            if k != "version" {
                con.insert(k, x);
            }
        }
        *m = con;
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// **Lee** un `.pdplantilla` (o un JSON que lo parezca), reparando.
pub fn leer(texto: &str) -> Result<(Documento, Vec<String>), ErrorFormato> {
    let v = json_de(texto)?;
    if let Some(n) = v.get("version").and_then(numero)
        && n as u32 > VERSION_FORMATO
        && v.get("formato").and_then(Value::as_str) == Some(MARCA)
    {
        return Err(ErrorFormato::MasNueva(n as u32));
    }
    reparar(v)
}

/// **El JSON de un texto**: entero, o el primer objeto `{…}` que lleve
/// dentro (un modelo que contesta con «Aqui tienes: ```json …```»).
pub fn json_de(texto: &str) -> Result<Value, ErrorFormato> {
    let t = texto.trim().trim_start_matches('\u{feff}');
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Ok(v);
    }
    // El primer `{` con su `}` emparejado (sin contar los de las cadenas).
    let bytes = t.as_bytes();
    let Some(inicio) = t.find('{') else {
        return Err(ErrorFormato::NoEsJson("sin objeto".into()));
    };
    let (mut hondo, mut en_cadena, mut escapado) = (0i32, false, false);
    for (i, b) in bytes.iter().enumerate().skip(inicio) {
        if en_cadena {
            match (*b, escapado) {
                (_, true) => escapado = false,
                (b'\\', false) => escapado = true,
                (b'"', false) => en_cadena = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => en_cadena = true,
            b'{' => hondo += 1,
            b'}' => {
                hondo -= 1;
                if hondo == 0 {
                    return serde_json::from_str(&t[inicio..=i])
                        .map_err(|e| ErrorFormato::NoEsJson(e.to_string()));
                }
            }
            _ => {}
        }
    }
    Err(ErrorFormato::NoEsJson("objeto sin cerrar".into()))
}

fn numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
    .filter(|x: &f64| x.is_finite())
}

fn booleano(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => Some(n.as_f64().unwrap_or(0.0) != 0.0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "si" | "sí" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn texto(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.chars().take(MAX_TEXTO).collect()),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(m) => [
            "texto", "text", "titulo", "title", "nombre", "name", "label",
        ]
        .iter()
        .find_map(|k| m.get(*k).and_then(texto)),
        _ => None,
    }
}

fn primero<'a>(m: &'a Map<String, Value>, claves: &[&str]) -> Option<&'a Value> {
    claves.iter().find_map(|k| m.get(*k))
}

/// **Repara un documento** (o un bloque suelto, o una lista de bloques, que
/// se meten en una columna).
pub fn reparar(v: Value) -> Result<(Documento, Vec<String>), ErrorFormato> {
    let mut avisos = Vec::new();
    let mut doc = Documento::nuevo("");
    let Value::Object(m) = &v else {
        // Una lista o un texto: los bloques de la raiz.
        let mut cuenta = 0;
        let raiz = bloque(&v, 0, &mut cuenta, &mut avisos).ok_or(ErrorFormato::Vacio)?;
        doc.raiz = envolver(raiz);
        doc.numerar();
        return Ok((doc, avisos));
    };
    let raiz_v = primero(
        m,
        &[
            "raiz",
            "root",
            "bloques",
            "blocks",
            "contenido",
            "content",
            "layout",
            "body",
        ],
    );
    let es_doc = raiz_v.is_some() || m.contains_key("pagina") || m.contains_key("page");
    if let Some(n) = primero(m, &["nombre", "name", "titulo_plantilla", "title"]).and_then(texto)
        && es_doc
    {
        doc.nombre = n.chars().take(120).collect();
    }
    if let Some(p) = primero(m, &["pagina", "page"]) {
        doc.pagina = pagina(p, &mut avisos);
    }
    if let Some(t) = primero(m, &["tema", "theme", "estilo", "style"]) {
        doc.tema = tema(t, &mut avisos);
    }
    if let Some(i) = primero(m, &["idioma", "language", "locale", "lang"]).and_then(texto) {
        doc.idioma = crate::locale::de(&i).etiqueta.to_string();
    }
    if let Some(r) = primero(m, &["repeticion", "repeat", "repetition", "recurrence"]) {
        doc.repeticion = repeticion(r, &mut avisos);
    }
    let mut cuenta = 0;
    let raiz = match raiz_v {
        Some(r) => bloque(r, 0, &mut cuenta, &mut avisos),
        None if !es_doc => bloque(&v, 0, &mut cuenta, &mut avisos),
        None => None,
    }
    .ok_or(ErrorFormato::Vacio)?;
    doc.raiz = envolver(raiz);
    if cuenta >= MAX_BLOQUES {
        avisos.push(format!("mas de {MAX_BLOQUES} bloques: el resto se quito"));
    }
    doc.numerar();
    Ok((doc, avisos))
}

/// La raiz siempre es un contenedor: una hoja suelta va en una columna.
fn envolver(b: Bloque) -> Bloque {
    if b.tipo.es_contenedor() {
        b
    } else {
        Bloque::nuevo(Tipo::Columna {
            hijos: vec![b],
            hueco: HUECO,
        })
    }
}

fn pagina(v: &Value, avisos: &mut Vec<String>) -> Pagina {
    let mut p = Pagina::default();
    let Value::Object(m) = v else {
        if let Some(t) = texto(v).and_then(|s| tamano(&s)) {
            p.tamano = t;
        }
        return p;
    };
    if let Some(t) = primero(m, &["tamano", "tamaño", "size", "papel", "paper", "format"]) {
        match texto(t).and_then(|s| tamano(&s)) {
            Some(t) => p.tamano = t,
            None => {
                if let Value::Object(mm) = t
                    && let (Some(a), Some(b)) = (
                        primero(mm, &["ancho", "width", "w"]).and_then(numero),
                        primero(mm, &["alto", "height", "h"]).and_then(numero),
                    )
                {
                    p.tamano = Tamano::Medida {
                        ancho: (a as f32).clamp(40.0, 1200.0),
                        alto: (b as f32).clamp(40.0, 1200.0),
                    };
                } else {
                    avisos.push("tamano de pagina desconocido: A4".into());
                }
            }
        }
    }
    if let Some(h) = primero(m, &["horizontal", "landscape", "apaisada"]).and_then(booleano) {
        p.horizontal = h;
    }
    if let Some(o) = primero(m, &["orientacion", "orientation"]).and_then(texto) {
        p.horizontal = matches!(
            o.to_lowercase().as_str(),
            "horizontal" | "landscape" | "apaisada"
        );
    }
    if let Some(mg) = primero(m, &["margenes", "margins", "margen", "margin"]) {
        if let Some(x) = numero(mg) {
            p.margenes = crate::modelo::Margenes::iguales((x as f32).clamp(0.0, 60.0));
        } else if let Value::Object(mm) = mg {
            let d = p.margenes;
            let lee = |ks: &[&str], def: f32| {
                primero(mm, ks)
                    .and_then(numero)
                    .map_or(def, |x| (x as f32).clamp(0.0, 60.0))
            };
            p.margenes.arriba = lee(&["arriba", "top"], d.arriba);
            p.margenes.abajo = lee(&["abajo", "bottom"], d.abajo);
            p.margenes.izquierda = lee(&["izquierda", "left"], d.izquierda);
            p.margenes.derecha = lee(&["derecha", "right"], d.derecha);
        }
    }
    p
}

fn tamano(s: &str) -> Option<Tamano> {
    Some(
        match s
            .trim()
            .to_lowercase()
            .replace([' ', '-', '_'], "")
            .as_str()
        {
            "a4" => Tamano::A4,
            "a5" => Tamano::A5,
            "carta" | "letter" | "usletter" => Tamano::Carta,
            "tableta" | "tablet" | "ipad" => Tamano::Tableta,
            "pantalla" | "screen" | "16:9" | "desktop" => Tamano::Pantalla,
            _ => return None,
        },
    )
}

fn tema(v: &Value, avisos: &mut Vec<String>) -> Tema {
    if let Some(s) = v.as_str() {
        return tema_de_nombre(s).unwrap_or_else(|| {
            avisos.push(format!("tema «{s}» desconocido"));
            Tema::default()
        });
    }
    // Uno entero, o uno de serie con algun color cambiado.
    if let Ok(t) = serde_json::from_value::<Tema>(v.clone()) {
        return t;
    }
    let Value::Object(m) = v else {
        return Tema::default();
    };
    let mut t = primero(m, &["base", "nombre", "name", "preset"])
        .and_then(texto)
        .and_then(|s| tema_de_nombre(&s))
        .unwrap_or_default();
    if let Some(c) = primero(m, &["acento", "accent", "color"])
        .and_then(texto)
        .and_then(|s| Color::leer(&s))
    {
        t.acento = c;
        t.suave = c.mezcla(Color::hex(0xffffff), 0.88);
        t.base = "propio".into();
    }
    if let Some(b) = primero(m, &["banda", "band"]).and_then(booleano) {
        t.banda = b;
    }
    if let Some(g) = primero(m, &["grosor", "line_weight", "stroke"]).and_then(numero) {
        t.grosor = (g as f32).clamp(0.1, 1.5);
    }
    t
}

fn tema_de_nombre(s: &str) -> Option<Tema> {
    let s = s.trim().to_lowercase();
    let clave = match s.as_str() {
        "planning" | "planner" | "planificación" | "azul" | "blue" => "planificacion",
        "academic" | "académico" | "verde" | "green" | "study" => "academico",
        "creative" | "naranja" | "orange" => "creativo",
        "journal" | "diary" | "rosa" | "pink" => "diario",
        "minimal" | "mínimo" | "simple" => "minimo",
        "night" | "dark" | "oscuro" => "noche",
        otro => otro,
    };
    Tema::de_serie(clave)
}

fn repeticion(v: &Value, avisos: &mut Vec<String>) -> Option<Repeticion> {
    let m = v.as_object()?;
    let cada = primero(
        m,
        &[
            "cada",
            "every",
            "each",
            "por",
            "per",
            "frecuencia",
            "frequency",
        ],
    )
    .and_then(texto)?
    .to_lowercase();
    let cada = match cada.trim() {
        "dia" | "día" | "day" | "daily" | "diaria" | "diario" => Cada::Dia,
        "semana" | "week" | "weekly" | "semanal" => Cada::Semana,
        "mes" | "month" | "monthly" | "mensual" => Cada::Mes,
        "ninguna" | "none" | "" => return None,
        otro => {
            avisos.push(format!("repeticion «{otro}» desconocida"));
            return None;
        }
    };
    let fecha = |ks: &[&str]| {
        primero(m, ks)
            .and_then(texto)
            .and_then(|s| crate::fecha::Fecha::leer(&s))
    };
    let mut desde = fecha(&["desde", "from", "start", "inicio"]);
    let mut hasta = fecha(&["hasta", "to", "end", "fin"]);
    // «Todo 2027»: un año suelto.
    if let Some(a) = primero(m, &["ano", "año", "year"]).and_then(numero) {
        let a = a as i32;
        desde = desde.or(crate::fecha::Fecha::nueva(a, 1, 1));
        hasta = hasta.or(crate::fecha::Fecha::nueva(a, 12, 31));
    }
    let (Some(desde), Some(hasta)) = (desde, hasta) else {
        avisos.push("repeticion sin fechas: se quito".into());
        return None;
    };
    let (desde, hasta) = if desde <= hasta {
        (desde, hasta)
    } else {
        (hasta, desde)
    };
    Some(Repeticion {
        cada,
        desde: desde.to_string(),
        hasta: hasta.to_string(),
        sin_fines_de_semana: primero(
            m,
            &["sin_fines_de_semana", "weekdays_only", "skip_weekends"],
        )
        .and_then(booleano)
        .unwrap_or(false),
    })
}

/// El tipo canonico de un nombre (el nuestro o el que suele usar un modelo).
pub fn tipo_canonico(s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase().replace(['-', ' '], "_");
    Some(match s.as_str() {
        "fila" | "row" | "hstack" | "horizontal" => "fila",
        "columna" | "column" | "col" | "vstack" | "vertical" | "stack_vertical" => "columna",
        "pila" | "stack" | "overlay" | "zstack" | "capas" => "pila",
        "rejilla" | "grid" | "cuadricula_bloques" => "rejilla",
        "seccion" | "sección" | "section" | "box" | "caja" | "group" | "grupo" | "card" => {
            "seccion"
        }
        "encabezado" | "header" | "heading" | "title" | "titulo" | "título" | "h1" | "h2"
        | "h3" => "encabezado",
        "texto" | "text" | "paragraph" | "parrafo" | "párrafo" | "label" | "etiqueta" => "texto",
        "notas" | "notes" | "note" | "nota" | "notes_area" => "notas",
        "lineas" | "líneas" | "lines" | "ruled" | "rayado" | "dotted" | "puntos" | "dots"
        | "grid_paper" | "cuadricula" | "cuadrícula" | "blank" | "blanco" => "lineas",
        "vinetas" | "viñetas" | "bullets" | "bullet_list" | "list" | "lista" | "ul" => "vinetas",
        "casillas" | "checkbox" | "checkboxes" | "checklist" | "todo" | "to_do" | "todo_list"
        | "tareas" | "tasks" => "casillas",
        "numerada" | "numbered" | "numbered_list" | "ol" | "lista_numerada" => "numerada",
        "tabla" | "table" => "tabla",
        "divisor" | "divider" | "separator" | "separador" | "hr" | "line" | "linea" => "divisor",
        "espacio" | "spacer" | "space" | "gap" => "espacio",
        "imagen" | "image" | "photo" | "foto" | "picture" => "imagen",
        "campo" | "field" | "date" | "fecha" | "dynamic_field" => "campo",
        "calendario_mes" | "calendario" | "calendar" | "month" | "month_calendar" | "mes"
        | "monthly_calendar" => "calendario_mes",
        "calendario_semana" | "week" | "week_calendar" | "weekly" | "semana"
        | "weekly_calendar" => "calendario_semana",
        "horario" | "schedule" | "timetable" | "time_slots" | "hours" | "horas" | "agenda"
        | "day_schedule" => "horario",
        "habitos" | "hábitos" | "habits" | "habit_tracker" | "tracker" => "habitos",
        "animo" | "ánimo" | "mood" | "estado_de_animo" | "mood_tracker" => "animo",
        "prioridades" | "priorities" | "priority" | "prioridad" => "prioridades",
        "cornell" | "cornell_notes" => "cornell",
        "reunion" | "reunión" | "meeting" | "meeting_notes" | "acta" => "reunion",
        _ => return None,
    })
}

/// Los nombres de campo que se aceptan por cada uno nuestro.
const ALIAS: [(&str, &[&str]); 29] = [
    ("texto", &["text", "content", "value", "label"]),
    ("titulo", &["title", "label", "name", "nombre", "heading"]),
    ("nivel", &["level"]),
    ("alineacion", &["align", "alignment"]),
    ("puntos", &["font_size", "fontsize", "size_pt", "pt"]),
    ("pauta", &["style", "pattern", "lines", "ruling", "estilo"]),
    ("paso", &["spacing", "step", "interval", "line_spacing"]),
    (
        "items",
        &["elementos", "lista", "list", "entries", "tasks", "tareas"],
    ),
    (
        "vacias",
        &[
            "empty",
            "blank",
            "extra",
            "empty_lines",
            "blank_lines",
            "lineas_vacias",
        ],
    ),
    ("filas", &["rows", "row_count"]),
    ("columnas", &["columns", "cols", "column_count"]),
    ("cabeceras", &["headers", "header", "head", "encabezados"]),
    ("hueco", &["gap", "spacing_mm"]),
    ("recuadro", &["border", "boxed", "frame"]),
    ("trazo", &["style", "stroke"]),
    ("etiqueta", &["label", "caption", "text"]),
    ("campo", &["field", "kind", "que"]),
    ("formato", &["format"]),
    ("antes", &["prefix", "before"]),
    ("despues", &["suffix", "after"]),
    ("negrita", &["bold"]),
    ("mes", &["month"]),
    ("mini", &["small", "compact"]),
    ("fin_de_semana", &["weekend", "weekends", "include_weekend"]),
    ("desde", &["from", "start", "start_hour", "inicio"]),
    ("hasta", &["to", "end", "end_hour", "fin"]),
    ("dias", &["days", "day_count"]),
    ("habitos", &["habits", "items", "rows"]),
    ("niveles", &["levels", "priorities", "items"]),
];

fn enumerado(campo: &str, s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase().replace(['-', ' '], "_");
    Some(match (campo, s.as_str()) {
        ("alineacion", "izquierda" | "left" | "start" | "inicio") => "izquierda",
        ("alineacion", "centro" | "center" | "centre" | "middle" | "centrado") => "centro",
        ("alineacion", "derecha" | "right" | "end" | "fin") => "derecha",
        ("pauta", "rayado" | "lined" | "ruled" | "lines" | "lineas" | "renglones") => "rayado",
        ("pauta", "puntos" | "dots" | "dotted" | "dot_grid" | "punteado") => "puntos",
        ("pauta", "cuadricula" | "cuadrícula" | "grid" | "grid_paper" | "squared" | "graph") => {
            "cuadricula"
        }
        ("pauta", "blanco" | "blank" | "none" | "plain" | "vacio") => "blanco",
        ("trazo", "continuo" | "solid" | "line" | "solido") => "continuo",
        ("trazo", "discontinuo" | "dashed" | "dash") => "discontinuo",
        ("trazo", "punteado" | "dotted" | "dots") => "punteado",
        ("trazo", "acento" | "accent" | "thick" | "grueso") => "acento",
        ("campo", "fecha" | "date") => "fecha",
        ("campo", "dia" | "día" | "day" | "day_of_month") => "dia",
        ("campo", "dia_semana" | "día_semana" | "weekday" | "day_of_week" | "dia_de_la_semana") => {
            "dia_semana"
        }
        ("campo", "semana" | "week" | "week_number" | "numero_de_semana") => "semana",
        ("campo", "mes" | "month") => "mes",
        ("campo", "ano" | "año" | "year") => "ano",
        ("campo", "pagina" | "página" | "page" | "page_number") => "pagina",
        ("formato", "corto" | "short") => "corto",
        ("formato", "largo" | "long") => "largo",
        ("formato", "completo" | "full") => "completo",
        ("formato", "iso") => "iso",
        ("formato", "numero" | "número" | "number" | "numeric") => "numero",
        ("formato", "abreviado" | "abbr" | "abbreviated") => "abreviado",
        ("formato", "letra" | "letter" | "initial") => "letra",
        _ => return None,
    })
}

/// **Los campos de cada tipo** y como se leen (la misma tabla para el
/// panel de propiedades, `paleta::de`).
pub fn campos_de(tipo: &str) -> &'static [(&'static str, Clase)] {
    use Clase::*;
    match tipo {
        "fila" | "columna" => &[("hueco", Real(0.0, 30.0))],
        "rejilla" => &[("columnas", Entero(1, 12)), ("hueco", Real(0.0, 30.0))],
        "seccion" => &[("titulo", Texto), ("recuadro", Si)],
        "encabezado" => &[
            ("texto", Texto),
            ("nivel", Entero(1, 3)),
            ("alineacion", Enum),
        ],
        "texto" => &[
            ("texto", Texto),
            ("puntos", Real(6.0, 72.0)),
            ("alineacion", Enum),
        ],
        "notas" => &[("titulo", Texto), ("pauta", Enum)],
        "lineas" => &[("pauta", Enum), ("paso", Real(3.0, 30.0))],
        "vinetas" | "casillas" | "numerada" => {
            &[("items", Lista), ("vacias", Entero(0, MAX_ITEMS as i64))]
        }
        "tabla" => &[
            ("filas", Entero(1, 80)),
            ("columnas", Entero(1, 20)),
            ("cabeceras", Lista),
        ],
        "divisor" => &[("trazo", Enum)],
        "imagen" => &[("etiqueta", Texto)],
        "campo" => &[
            ("campo", Enum),
            ("formato", Enum),
            ("antes", Texto),
            ("despues", Texto),
            ("puntos", Real(6.0, 96.0)),
            ("alineacion", Enum),
            ("negrita", Si),
        ],
        "calendario_mes" => &[("mes", Texto), ("mini", Si)],
        "calendario_semana" => &[("fin_de_semana", Si)],
        "horario" => &[
            ("desde", Entero(0, 23)),
            ("hasta", Entero(1, 24)),
            ("paso", Entero(15, 60)),
            ("dias", Entero(1, 7)),
        ],
        "habitos" => &[("habitos", Lista), ("dias", Entero(0, 31))],
        "animo" => &[("titulo", Texto)],
        "prioridades" => &[("niveles", Lista)],
        "cornell" | "reunion" => &[("titulo", Texto), ("etiquetas", Lista)],
        _ => &[],
    }
}

#[derive(Clone, Copy)]
pub enum Clase {
    Texto,
    Entero(i64, i64),
    Real(f64, f64),
    Si,
    Lista,
    Enum,
}

/// **Un bloque** de un valor cualquiera, reparado. `None` si no hay nada
/// que se entienda.
pub fn bloque(
    v: &Value,
    hondo: usize,
    cuenta: &mut usize,
    avisos: &mut Vec<String>,
) -> Option<Bloque> {
    if *cuenta >= MAX_BLOQUES {
        return None;
    }
    if hondo > MAX_HONDO {
        avisos.push(format!("mas de {MAX_HONDO} niveles: lo de dentro se quito"));
        return None;
    }
    match v {
        Value::Array(lista) => {
            *cuenta += 1;
            let hijos: Vec<Bloque> = lista
                .iter()
                .filter_map(|h| bloque(h, hondo + 1, cuenta, avisos))
                .collect();
            Some(Bloque::nuevo(Tipo::Columna {
                hijos,
                hueco: HUECO,
            }))
        }
        Value::String(s) if !s.trim().is_empty() => {
            *cuenta += 1;
            Some(crate::modelo::hacer::texto(
                &s.chars().take(MAX_TEXTO).collect::<String>(),
            ))
        }
        Value::Object(m) => objeto(m, hondo, cuenta, avisos),
        _ => None,
    }
}

fn objeto(
    m: &Map<String, Value>,
    hondo: usize,
    cuenta: &mut usize,
    avisos: &mut Vec<String>,
) -> Option<Bloque> {
    let nombre =
        primero(m, &["tipo", "type", "kind", "block", "bloque", "component"]).and_then(texto);
    let Some(tipo) = nombre.as_deref().and_then(tipo_canonico) else {
        // Sin tipo que se entienda: si tiene hijos, una columna; si tiene
        // texto, un texto; si no, fuera.
        if let Some(h) = primero(m, &["hijos", "children", "blocks", "bloques"]) {
            let mut b = bloque(h, hondo, cuenta, avisos)?;
            b.tam = tam_de(m);
            return Some(b);
        }
        if let Some(t) = primero(m, &["texto", "text", "titulo", "title"]).and_then(texto) {
            *cuenta += 1;
            avisos.push(format!(
                "tipo «{}» desconocido: queda como texto",
                nombre.unwrap_or_default()
            ));
            return Some(crate::modelo::hacer::texto(&t));
        }
        avisos.push(format!(
            "tipo «{}» desconocido: se quito",
            nombre.unwrap_or_default()
        ));
        return None;
    };
    *cuenta += 1;
    let mut limpio = Map::new();
    limpio.insert("tipo".into(), Value::String(tipo.into()));
    // Lo que el nombre ya decia (`h2`, `dotted`, `checklist`…).
    let pista = nombre.unwrap_or_default().to_lowercase();
    match (tipo, pista.as_str()) {
        ("encabezado", "h2") => {
            limpio.insert("nivel".into(), 2.into());
        }
        ("encabezado", "h3") => {
            limpio.insert("nivel".into(), 3.into());
        }
        ("lineas", p) if enumerado("pauta", p).is_some() => {
            limpio.insert(
                "pauta".into(),
                enumerado("pauta", p).unwrap_or("rayado").into(),
            );
        }
        _ => {}
    }
    for (campo, clase) in campos_de(tipo) {
        let mut claves: Vec<&str> = vec![campo];
        if let Some((_, a)) = ALIAS.iter().find(|(c, _)| c == campo) {
            claves.extend_from_slice(a);
        }
        let Some(valor) = primero(m, &claves) else {
            continue;
        };
        let bueno: Option<Value> = match clase {
            Clase::Texto => texto(valor).map(Value::String),
            Clase::Entero(a, b) => {
                numero(valor).map(|x| Value::from((x.round() as i64).clamp(*a, *b)))
            }
            Clase::Real(a, b) => numero(valor).map(|x| Value::from(x.clamp(*a, *b))),
            Clase::Si => booleano(valor).map(Value::Bool),
            Clase::Lista => match valor {
                Value::Array(l) => Some(Value::Array(
                    l.iter()
                        .filter_map(texto)
                        .take(MAX_ITEMS)
                        .map(Value::String)
                        .collect(),
                )),
                Value::String(s) => Some(Value::Array(
                    s.split([',', '\n', ';'])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .take(MAX_ITEMS)
                        .map(|x| Value::String(x.into()))
                        .collect(),
                )),
                // `"items": 5`: cinco renglones vacios.
                Value::Number(_) if *campo == "items" => {
                    if let Some(n) = numero(valor) {
                        limpio.insert(
                            "vacias".into(),
                            Value::from((n as i64).clamp(0, MAX_ITEMS as i64)),
                        );
                    }
                    None
                }
                _ => None,
            },
            Clase::Enum => texto(valor)
                .and_then(|s| enumerado(campo, &s))
                .map(|s| Value::String(s.into())),
        };
        match bueno {
            Some(b) => {
                limpio.insert((*campo).into(), b);
            }
            None if !matches!(valor, Value::Number(_)) || *campo != "items" => {
                avisos.push(format!("{tipo}.{campo}: valor que no se entiende"));
            }
            None => {}
        }
    }
    // Un horario al reves se da la vuelta.
    if tipo == "horario"
        && let (Some(a), Some(b)) = (
            limpio.get("desde").and_then(Value::as_i64),
            limpio.get("hasta").and_then(Value::as_i64),
        )
        && a >= b
    {
        limpio.insert("desde".into(), Value::from(b.min(a).min(23)));
        limpio.insert(
            "hasta".into(),
            Value::from((a.max(b)).max(b.min(a) + 1).min(24)),
        );
    }
    let mut b: Bloque = match serde_json::from_value(Value::Object(limpio)) {
        Ok(b) => b,
        Err(e) => {
            avisos.push(format!("{tipo}: {e}"));
            return None;
        }
    };
    if let Some(id) = m.get("id").and_then(numero) {
        b.id = id.max(0.0) as u64;
    }
    b.tam = tam_de(m);
    if let Some(c) = primero(m, &["acento", "accent", "color"])
        .and_then(texto)
        .and_then(|s| Color::leer(&s))
    {
        b.acento = Some(c);
    }
    if let Some(h) = b.tipo.hijos_mut() {
        if let Some(lista) = primero(
            m,
            &[
                "hijos",
                "children",
                "blocks",
                "bloques",
                "contenido",
                "content",
            ],
        )
        .or_else(|| {
            m.get("items")
                .filter(|v| v.as_array().is_some_and(|l| l.iter().any(Value::is_object)))
        }) && let Value::Array(l) = lista
        {
            for x in l {
                if let Some(c) = bloque(x, hondo + 1, cuenta, avisos) {
                    h.push(c);
                }
            }
        }
    }
    Some(b)
}

fn tam_de(m: &Map<String, Value>) -> Tam {
    match primero(
        m,
        &["tam", "tamano", "size", "flex", "width", "height", "basis"],
    ) {
        Some(Value::String(s)) => Tam::leer(s).unwrap_or_default(),
        Some(Value::Number(n)) => {
            let x = n.as_f64().unwrap_or(1.0) as f32;
            // Un numero pequeño suele ser un flex; uno grande, milimetros.
            if x <= 6.0 {
                Tam::Fraccion(x.max(0.05))
            } else {
                Tam::Fijo(x.min(2000.0))
            }
        }
        _ => Tam::Auto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn los_nombres_en_ingles_y_los_numeros_como_texto_se_entienden() {
        let v = json!({
            "name": "Weekly",
            "page": {"size": "letter", "orientation": "landscape"},
            "theme": "journal",
            "root": {"type": "column", "children": [
                {"type": "h2", "text": "Semana {semana}"},
                {"type": "row", "children": [
                    {"type": "checklist", "items": ["Pan", {"text": "Leche"}], "empty": "3", "size": "2fr"},
                    {"type": "schedule", "from": "22", "to": 7, "step": 30, "width": "60mm"},
                ]},
                {"type": "grid_paper"},
                {"type": "nada que ver"},
            ]},
        });
        let (d, avisos) = reparar(v).unwrap();
        assert_eq!(d.nombre, "Weekly");
        assert_eq!(d.pagina.tamano, Tamano::Carta);
        assert!(d.pagina.horizontal);
        assert_eq!(d.tema.base, "diario");
        let h = d.raiz.tipo.hijos().unwrap();
        assert_eq!(h.len(), 3, "lo desconocido se quita");
        assert!(matches!(&h[0].tipo, Tipo::Encabezado { nivel: 2, .. }));
        let fila = h[1].tipo.hijos().unwrap();
        match &fila[0].tipo {
            Tipo::Casillas { items, vacias } => {
                assert_eq!(items, &vec!["Pan".to_string(), "Leche".to_string()]);
                assert_eq!(*vacias, 3);
            }
            otro => panic!("{otro:?}"),
        }
        assert_eq!(fila[0].tam, Tam::Fraccion(2.0));
        match &fila[1].tipo {
            Tipo::Horario {
                desde, hasta, paso, ..
            } => assert_eq!((*desde, *hasta, *paso), (7, 22, 30)),
            otro => panic!("{otro:?}"),
        }
        assert_eq!(fila[1].tam, Tam::Fijo(60.0));
        assert!(matches!(
            h[2].tipo,
            Tipo::Lineas {
                pauta: crate::modelo::Pauta::Cuadricula,
                ..
            }
        ));
        assert!(avisos.iter().any(|a| a.contains("nada que ver")));
        // Todos con id y distintos.
        let mut ids = Vec::new();
        d.raiz.recorrer(&mut |b, _| ids.push(b.id));
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), d.cuantos());
    }

    #[test]
    fn json_dentro_de_un_texto_y_casos_que_no_son() {
        let t = "Claro, aqui la tienes:\n```json\n{\"tipo\": \"tabla\", \"filas\": 400, \"columnas\": \"4\", \"texto\": \"a}b\"}\n```\nEspero que sirva";
        let (d, _) = leer(t).unwrap();
        match &d.raiz.tipo.hijos().unwrap()[0].tipo {
            Tipo::Tabla {
                filas, columnas, ..
            } => assert_eq!((*filas, *columnas), (80, 4)),
            otro => panic!("{otro:?}"),
        }
        assert!(matches!(leer("sin json"), Err(ErrorFormato::NoEsJson(_))));
        assert!(matches!(
            leer("{\"tipo\": \"tabla\""),
            Err(ErrorFormato::NoEsJson(_))
        ));
        assert_eq!(
            leer("{\"tipo\": \"zzz\"}").unwrap_err(),
            ErrorFormato::Vacio
        );
        let nueva = format!("{{\"formato\": \"{MARCA}\", \"version\": 99, \"raiz\": []}}");
        assert_eq!(leer(&nueva).unwrap_err(), ErrorFormato::MasNueva(99));
    }

    #[test]
    fn un_arbol_enorme_o_muy_hondo_se_corta() {
        let mut v = json!({"tipo": "texto", "texto": "hondo"});
        for _ in 0..30 {
            v = json!({"tipo": "columna", "hijos": [v]});
        }
        let (d, avisos) = reparar(v).unwrap();
        let mut hondo = 0;
        d.raiz.recorrer(&mut |_, p| hondo = hondo.max(p));
        assert!(hondo <= MAX_HONDO + 1);
        assert!(avisos.iter().any(|a| a.contains("niveles")));
        let muchos: Vec<Value> = (0..1000)
            .map(|i| json!({"tipo": "texto", "texto": i.to_string()}))
            .collect();
        let (d, avisos) = reparar(json!({"raiz": muchos})).unwrap();
        assert!(d.cuantos() <= MAX_BLOQUES + 1);
        assert!(avisos.iter().any(|a| a.contains("bloques")));
    }

    #[test]
    fn la_repeticion_de_un_ano_suelto() {
        let v = json!({"raiz": [{"type": "date"}], "repeat": {"every": "daily", "year": 2027}});
        let (d, _) = reparar(v).unwrap();
        let r = d.repeticion.unwrap();
        assert_eq!(
            (r.cada, r.desde.as_str(), r.hasta.as_str()),
            (Cada::Dia, "2027-01-01", "2027-12-31")
        );
    }
}
