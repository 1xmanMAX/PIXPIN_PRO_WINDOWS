//! **La IA**: «descríbelo» (de una frase a una plantilla), «ajustar la
//! seleccion» y «sugerir un relleno».
//!
//! El modelo **no escribe texto libre que se lea a mano**: se le da una sola
//! herramienta (`poner_plantilla` o `poner_bloque`) con su esquema JSON y lo
//! que devuelve pasa por la misma reparacion que un fichero
//! (`formato::reparar`): nombres en ingles, numeros como texto, topes,
//! profundidad… Si aun asi no hay JSON, se le pide una vez mas diciendole
//! que fallo. Lo que sale se enseña como vista previa y solo se aplica si el
//! usuario quiere.
//!
//! Quien habla con el modelo es un [`ClienteIa`]: en Windows, el de
//! `pindesk-ia` (Claude o DeepSeek con la clave y el modelo de PinDesk); en
//! las pruebas, [`ClienteFalso`]. Sin clave, [`sin_modelo`] hace una
//! plantilla razonable con las palabras de la frase (horario de 7 a 22,
//! tareas, habitos, semanal…), sin red.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::fecha::Fecha;
use crate::formato;
use crate::locale::FormatoFecha;
use crate::modelo::{Bloque, Cada, Documento, Pauta, QueCampo, Repeticion, Tam, Tipo, hacer};
use crate::textos::Textos;

/// El nombre de la herramienta para una plantilla entera.
pub const HERRAMIENTA_PLANTILLA: &str = "poner_plantilla";
/// Para un bloque.
pub const HERRAMIENTA_BLOQUE: &str = "poner_bloque";
/// Las veces que se le pide al modelo (la primera y una de arreglo).
pub const INTENTOS: usize = 2;

/// **Lo que se le pide al modelo.**
#[derive(Debug, Clone, PartialEq)]
pub struct Pedido {
    pub sistema: String,
    pub usuario: String,
    /// La unica herramienta que se le ofrece.
    pub herramienta: &'static str,
    pub descripcion: String,
    /// El esquema JSON de sus argumentos.
    pub esquema: Value,
}

/// **Quien habla con el modelo.** Devuelve el JSON de los argumentos de la
/// herramienta (o, si el modelo contesto con texto, ese texto: se busca el
/// JSON dentro).
pub trait ClienteIa: Send + Sync {
    fn pedir(&self, p: &Pedido) -> Result<String, String>;
}

/// Un cliente de mentira que contesta lo que se le dice, en orden.
#[derive(Default)]
pub struct ClienteFalso {
    respuestas: Mutex<VecDeque<Result<String, String>>>,
    /// Lo que se le pidio.
    pub recibidos: Mutex<Vec<Pedido>>,
}

impl ClienteFalso {
    pub fn con(respuestas: &[&str]) -> ClienteFalso {
        ClienteFalso {
            respuestas: Mutex::new(respuestas.iter().map(|s| Ok(s.to_string())).collect()),
            recibidos: Mutex::new(Vec::new()),
        }
    }

    pub fn fallando(motivo: &str) -> ClienteFalso {
        ClienteFalso {
            respuestas: Mutex::new(VecDeque::from([Err(motivo.to_string())])),
            recibidos: Mutex::new(Vec::new()),
        }
    }

    pub fn cuantos(&self) -> usize {
        self.recibidos.lock().map_or(0, |r| r.len())
    }
}

impl ClienteIa for ClienteFalso {
    fn pedir(&self, p: &Pedido) -> Result<String, String> {
        if let Ok(mut r) = self.recibidos.lock() {
            r.push(p.clone());
        }
        self.respuestas
            .lock()
            .ok()
            .and_then(|mut r| r.pop_front())
            .unwrap_or_else(|| Err("sin mas respuestas".into()))
    }
}

/// Por que no salio.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorIa {
    /// El cliente fallo (sin red, sin clave, el servidor dijo que no).
    Cliente(String),
    /// Contesto, pero no con una plantilla que se pueda usar.
    Respuesta(String),
}

impl std::fmt::Display for ErrorIa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorIa::Cliente(e) => write!(f, "{e}"),
            ErrorIa::Respuesta(e) => write!(f, "respuesta sin plantilla: {e}"),
        }
    }
}

/// **Una propuesta** para enseñar antes de aplicar.
#[derive(Debug, Clone, PartialEq)]
pub struct Propuesta {
    pub doc: Documento,
    /// Lo que la reparacion tuvo que cambiar.
    pub avisos: Vec<String>,
}

// ---------------------------------------------------------------------------
// Lo que se le dice
// ---------------------------------------------------------------------------

/// El catalogo de bloques tal como se le explica al modelo.
const CATALOGO: &str = r#"Bloques (campo "tipo"); todos admiten "tam": "auto" | "<n>fr" | "<n>mm" (tamaño en el eje del contenedor):
- Contenedores (con "hijos": [...]):
  fila {hueco}: hijos uno al lado del otro. columna {hueco}: uno debajo del otro.
  rejilla {columnas, hueco}: en rejilla por filas. pila: superpuestos.
  seccion {titulo, recuadro}: rotulo con raya de acento y los hijos debajo.
- Texto: encabezado {texto, nivel 1-3, alineacion izquierda|centro|derecha}; texto {texto, puntos, alineacion};
  notas {titulo, pauta rayado|puntos|cuadricula|blanco}; lineas {pauta, paso mm}; divisor {trazo continuo|discontinuo|punteado|acento}; espacio; imagen {etiqueta}.
- Listas: vinetas|casillas|numerada {items: [textos], vacias: renglones vacios}; tabla {filas, columnas, cabeceras: [textos]}.
- Fechas (se llenan solas en cada pagina): campo {campo fecha|dia|dia_semana|semana|mes|ano|pagina, formato corto|largo|completo|iso|numero|abreviado|letra, antes, despues, puntos, negrita};
  calendario_mes {mes "AAAA-MM" o vacio = el de la pagina, mini}; calendario_semana {fin_de_semana}; horario {desde hora, hasta hora, paso 15|30|60, dias 1|5|7}.
- Especiales: habitos {habitos: [textos], dias 0 = el mes, 7 = la semana}; animo {titulo}; prioridades {niveles: [textos]}; cornell {titulo, etiquetas: [claves, notas, resumen]}; reunion {titulo, etiquetas: [fecha, asistentes, orden del dia, acuerdos, tareas]}.
Los textos pueden llevar campos: {fecha}, {fecha:largo}, {fecha:completo}, {dia}, {dia_semana}, {semana}, {mes}, {ano}, {pagina}, {paginas}."#;

fn instrucciones(idioma: &str, hoy: Fecha) -> String {
    format!(
        "Eres el diseñador de {producto}. Haces plantillas imprimibles bonitas y utiles \
(planificadores, diarios, calendarios, listas, notas) con bloques. Contesta SIEMPRE llamando a la \
herramienta, nunca con texto. Usa solo los tipos y campos de abajo. Pon los textos en el idioma \
«{idioma}». Hoy es {hoy}. Diseño: un encabezado de nivel 1 arriba, secciones con titulo, filas para \
poner cosas lado a lado, «fr» para repartir el sitio y «mm» solo para lo que deba medir algo fijo; \
deja aire y no metas mas de 25 bloques. Para «un dia por pagina», «semanal» o «mensual» pon \
«repeticion» {{cada: dia|semana|mes, desde, hasta}} en AAAA-MM-DD.\n\n{CATALOGO}",
        producto = crate::NOMBRE_PRODUCTO,
    )
}

/// **El esquema** de la herramienta de una plantilla.
pub fn esquema_plantilla() -> Value {
    json!({
        "type": "object",
        "properties": {
            "nombre": {"type": "string"},
            "pagina": {
                "type": "object",
                "properties": {
                    "tamano": {"type": "string", "enum": ["a4", "a5", "carta", "tableta", "pantalla"]},
                    "horizontal": {"type": "boolean"}
                }
            },
            "tema": {"type": "string", "enum": crate::tema::Tema::claves().collect::<Vec<_>>()},
            "repeticion": {
                "type": "object",
                "properties": {
                    "cada": {"type": "string", "enum": ["dia", "semana", "mes"]},
                    "desde": {"type": "string"},
                    "hasta": {"type": "string"}
                }
            },
            "raiz": esquema_bloque()
        },
        "required": ["nombre", "raiz"]
    })
}

/// El esquema de un bloque (abierto: los campos de cada tipo van en las
/// instrucciones, y la reparacion manda).
pub fn esquema_bloque() -> Value {
    let tipos: Vec<&str> = [
        "fila",
        "columna",
        "rejilla",
        "pila",
        "seccion",
        "encabezado",
        "texto",
        "notas",
        "lineas",
        "vinetas",
        "casillas",
        "numerada",
        "tabla",
        "divisor",
        "espacio",
        "imagen",
        "campo",
        "calendario_mes",
        "calendario_semana",
        "horario",
        "habitos",
        "animo",
        "prioridades",
        "cornell",
        "reunion",
    ]
    .to_vec();
    json!({
        "type": "object",
        "description": "Un bloque. Los contenedores llevan \"hijos\" (lista de bloques).",
        "properties": {
            "tipo": {"type": "string", "enum": tipos},
            "tam": {"type": "string"},
            "hijos": {"type": "array", "items": {"type": "object"}}
        },
        "required": ["tipo"],
        "additionalProperties": true
    })
}

fn pedir_con_arreglo(cliente: &dyn ClienteIa, mut pedido: Pedido) -> Result<Value, ErrorIa> {
    let mut ultimo = String::new();
    for _ in 0..INTENTOS {
        let r = cliente.pedir(&pedido).map_err(ErrorIa::Cliente)?;
        match formato::json_de(&r) {
            Ok(v) => return Ok(v),
            Err(e) => {
                ultimo = e.to_string();
                pedido.usuario = format!(
                    "{}\n\nTu respuesta anterior no era JSON valido ({ultimo}). Llama a la herramienta {} con el JSON.",
                    pedido.usuario, pedido.herramienta
                );
            }
        }
    }
    Err(ErrorIa::Respuesta(ultimo))
}

/// **«Descríbelo»**: de una frase a una plantilla.
pub fn describir(
    cliente: &dyn ClienteIa,
    frase: &str,
    idioma: &str,
    hoy: Fecha,
) -> Result<Propuesta, ErrorIa> {
    let pedido = Pedido {
        sistema: instrucciones(idioma, hoy),
        usuario: frase.trim().to_string(),
        herramienta: HERRAMIENTA_PLANTILLA,
        descripcion: "Pone la plantilla diseñada (el documento entero).".into(),
        esquema: esquema_plantilla(),
    };
    let v = pedir_con_arreglo(cliente, pedido)?;
    let (mut doc, avisos) = formato::reparar(v).map_err(|e| ErrorIa::Respuesta(e.to_string()))?;
    if doc.nombre.trim().is_empty() {
        doc.nombre = frase.chars().take(60).collect();
    }
    doc.idioma = crate::locale::de(idioma).etiqueta.to_string();
    Ok(Propuesta { doc, avisos })
}

fn pedido_de_bloque(doc: &Documento, b: &Bloque, orden: &str, hoy: Fecha) -> Pedido {
    let actual = serde_json::to_string(b).unwrap_or_default();
    Pedido {
        sistema: instrucciones(&doc.idioma, hoy),
        usuario: format!(
            "Este es un bloque de la plantilla «{}»:\n{actual}\n\n{orden}\nDevuelve el bloque entero ya cambiado.",
            doc.nombre
        ),
        herramienta: HERRAMIENTA_BLOQUE,
        descripcion: "Pone el bloque cambiado.".into(),
        esquema: esquema_bloque(),
    }
}

fn bloque_de(v: Value) -> Result<(Bloque, Vec<String>), ErrorIa> {
    // Puede venir el bloque o un documento con el bloque en «raiz».
    let v = match v {
        Value::Object(mut m) if !m.contains_key("tipo") && !m.contains_key("type") => m
            .remove("raiz")
            .or_else(|| m.remove("bloque"))
            .or_else(|| m.remove("root"))
            .unwrap_or(Value::Object(m)),
        otro => otro,
    };
    let mut avisos = Vec::new();
    let mut cuenta = 0;
    let b = formato::bloque(&v, 0, &mut cuenta, &mut avisos)
        .ok_or_else(|| ErrorIa::Respuesta(avisos.join("; ")))?;
    Ok((b, avisos))
}

/// **«Ajustar la seleccion»** («mas filas», «ponlo en dos columnas»).
pub fn ajustar(
    cliente: &dyn ClienteIa,
    doc: &Documento,
    id: u64,
    orden: &str,
    hoy: Fecha,
) -> Result<(Bloque, Vec<String>), ErrorIa> {
    let b = doc
        .buscar(id)
        .ok_or_else(|| ErrorIa::Respuesta("sin bloque".into()))?;
    let v = pedir_con_arreglo(
        cliente,
        pedido_de_bloque(doc, b, &format!("Cambialo asi: {orden}"), hoy),
    )?;
    bloque_de(v)
}

/// **«Sugerir un relleno»**: textos utiles para los huecos del bloque.
pub fn sugerir(
    cliente: &dyn ClienteIa,
    doc: &Documento,
    id: u64,
    hoy: Fecha,
) -> Result<(Bloque, Vec<String>), ErrorIa> {
    let b = doc
        .buscar(id)
        .ok_or_else(|| ErrorIa::Respuesta("sin bloque".into()))?;
    let orden = "Rellena sus textos vacios (elementos de lista, cabeceras, habitos, titulos) con \
contenido util y breve que encaje con la plantilla. No cambies su tipo ni su estructura.";
    let v = pedir_con_arreglo(cliente, pedido_de_bloque(doc, b, orden, hoy))?;
    bloque_de(v)
}

// ---------------------------------------------------------------------------
// Sin modelo
// ---------------------------------------------------------------------------

fn tiene(frase: &str, palabras: &[&str]) -> bool {
    palabras.iter().any(|p| frase.contains(p))
}

/// Las horas «de 7 a 22» / «7-22» / «from 7 to 22».
fn horas(frase: &str) -> Option<(u32, u32)> {
    let numeros: Vec<u32> = frase
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .filter(|n| *n <= 24)
        .collect();
    match numeros.as_slice() {
        [a, b, ..] if a < b => Some((*a, *b)),
        _ => None,
    }
}

/// **Una plantilla con las palabras de la frase**, sin modelo ni red.
pub fn sin_modelo(frase: &str, idioma: &str, t: &Textos, hoy: Fecha) -> Documento {
    let f = frase.to_lowercase();
    let semanal = tiene(
        &f,
        &[
            "seman", "week", "hebdo", "woche", "settiman", "minggu", "hafta", "周", "週", "주",
        ],
    );
    let mensual = tiene(
        &f,
        &[
            "mensual", "mes ", "month", "mensuel", "monat", "mese", "bulan", "aylık", "月", "월",
        ],
    );
    let diario = tiene(
        &f,
        &[
            "diario", "dia", "día", "daily", "journal", "diary", "jour", "tag", "giorno", "hari",
            "gün", "日记",
        ],
    );
    let horario = tiene(
        &f,
        &[
            "horario", "hora", "schedule", "hour", "agenda", "horaire", "stunden", "orario",
            "jadwal", "saat",
        ],
    );
    let tareas = tiene(
        &f,
        &[
            "tarea",
            "pendiente",
            "to-do",
            "todo",
            "task",
            "tâche",
            "aufgabe",
            "attivit",
            "tugas",
            "görev",
        ],
    );
    let habitos = tiene(
        &f,
        &[
            "hábit",
            "habit",
            "gewohn",
            "abitud",
            "kebiasaan",
            "alışkanlık",
        ],
    );
    let notas = tiene(&f, &["nota", "note", "notiz", "catatan", "not "]);
    let cornell = f.contains("cornell");
    let reunion = tiene(
        &f,
        &[
            "reuni",
            "meeting",
            "réunion",
            "besprechung",
            "riunion",
            "rapat",
            "toplantı",
        ],
    );
    let (desde, hasta) = horas(&f).unwrap_or((7, 22));

    let mut doc = Documento::nuevo(
        &crate::diseno::capital(frase.trim())
            .chars()
            .take(60)
            .collect::<String>(),
    );
    doc.idioma = crate::locale::de(idioma).etiqueta.to_string();
    let mut hijos = Vec::new();
    if cornell {
        hijos.push(hacer::encabezado(&t.t("plt-def-titulo"), 1));
        hijos.push(Bloque::nuevo(Tipo::Cornell {
            titulo: String::new(),
            etiquetas: t.lista("plt-def-cornell"),
        }));
    } else if reunion {
        hijos.push(Bloque::nuevo(Tipo::Reunion {
            titulo: t.t("plt-def-reunion"),
            etiquetas: t.lista("plt-def-reunion-rotulos"),
        }));
    } else if semanal {
        doc.tema = crate::tema::Tema::de_serie("planificacion").unwrap_or_default();
        hijos.push(cabecera(&t.t("plt-gal-semanal"), QueCampo::Semana));
        let mut cuerpo = Vec::new();
        if horario {
            cuerpo.push(hacer::horario(desde, hasta, 7).con_tam(Tam::Fraccion(2.0)));
        } else {
            cuerpo.push(
                Bloque::nuevo(Tipo::CalendarioSemana {
                    fin_de_semana: true,
                })
                .con_tam(Tam::Fraccion(2.0)),
            );
        }
        let mut lado = Vec::new();
        if tareas || !habitos {
            lado.push(hacer::seccion(
                &t.t("plt-def-tareas"),
                vec![hacer::casillas(&[], 10)],
            ));
        }
        if habitos {
            lado.push(hacer::seccion(
                &t.t("plt-pieza-habitos"),
                vec![hacer::habitos(
                    &t.lista("plt-def-habitos")
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    7,
                )],
            ));
        }
        if notas || lado.len() < 2 {
            lado.push(hacer::notas(&t.t("plt-def-notas")));
        }
        if horario {
            hijos.push(cuerpo.remove(0));
            hijos.push(hacer::fila(lado).con_tam(Tam::Fraccion(1.0)));
        } else {
            hijos.push(hacer::fila(vec![cuerpo.remove(0), hacer::columna(lado)]));
        }
        doc.repeticion = Some(Repeticion {
            cada: Cada::Semana,
            desde: Fecha::nueva(hoy.ano + 1, 1, 1).unwrap_or(hoy).to_string(),
            hasta: Fecha::nueva(hoy.ano + 1, 12, 31).unwrap_or(hoy).to_string(),
            sin_fines_de_semana: false,
        });
    } else if mensual {
        hijos.push(hacer::encabezado("{mes} {ano}", 1));
        hijos.push(
            Bloque::nuevo(Tipo::CalendarioMes {
                mes: String::new(),
                mini: false,
            })
            .con_tam(Tam::Fraccion(3.0)),
        );
        if habitos {
            hijos.push(hacer::seccion(
                &t.t("plt-pieza-habitos"),
                vec![hacer::habitos(
                    &t.lista("plt-def-habitos")
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    0,
                )],
            ));
        } else {
            hijos.push(hacer::notas(&t.t("plt-def-notas")));
        }
        doc.repeticion = Some(Repeticion {
            cada: Cada::Mes,
            desde: Fecha::nueva(hoy.ano + 1, 1, 1).unwrap_or(hoy).to_string(),
            hasta: Fecha::nueva(hoy.ano + 1, 12, 31).unwrap_or(hoy).to_string(),
            sin_fines_de_semana: false,
        });
    } else {
        // Un dia (o lo que no se reconozca: un planificador de un dia).
        doc.tema = crate::tema::Tema::de_serie(if diario { "diario" } else { "planificacion" })
            .unwrap_or_default();
        hijos.push(cabecera("{dia_semana}", QueCampo::Fecha));
        let mut izq = Vec::new();
        if horario || !diario {
            izq.push(hacer::horario(desde, hasta, 1));
        } else {
            izq.push(hacer::notas(&t.t("plt-def-notas")));
        }
        let mut der = vec![hacer::seccion(
            &t.t("plt-def-tareas"),
            vec![hacer::casillas(&[], 8)],
        )];
        if habitos {
            der.push(hacer::seccion(
                &t.t("plt-pieza-habitos"),
                vec![hacer::habitos(
                    &t.lista("plt-def-habitos")
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    7,
                )],
            ));
        }
        der.push(hacer::notas(&t.t("plt-def-notas")));
        hijos.push(hacer::fila(vec![
            hacer::columna(izq).con_tam(Tam::Fraccion(1.2)),
            hacer::columna(der),
        ]));
        if diario {
            doc.repeticion = Some(Repeticion {
                cada: Cada::Dia,
                desde: Fecha::nueva(hoy.ano + 1, 1, 1).unwrap_or(hoy).to_string(),
                hasta: Fecha::nueva(hoy.ano + 1, 12, 31).unwrap_or(hoy).to_string(),
                sin_fines_de_semana: false,
            });
        }
    }
    if let Some(h) = doc.raiz.tipo.hijos_mut() {
        *h = hijos;
    }
    doc.numerar();
    doc
}

/// Un titulo con la fecha a la derecha.
fn cabecera(titulo: &str, campo: QueCampo) -> Bloque {
    let mut c = hacer::campo(campo, FormatoFecha::Largo, 11.0);
    if let Tipo::Campo { alineacion, .. } = &mut c.tipo {
        *alineacion = crate::modelo::Alineacion::Derecha;
    }
    hacer::fila(vec![
        hacer::encabezado(titulo, 1),
        c.con_tam(Tam::Fraccion(0.8)),
    ])
}

/// **Ajustar sin modelo**: «mas filas», «menos», «dos columnas»,
/// «puntos», «cuadricula»… `None` si no se entiende la orden.
pub fn ajustar_sin_modelo(b: &Bloque, orden: &str) -> Option<Bloque> {
    let o = orden.to_lowercase();
    let mas = tiene(
        &o,
        &[
            "más", "mas ", "more", "plus", "mehr", "più", "lebih", "daha", "多", "더",
        ],
    ) || o.starts_with("mas");
    let menos = tiene(
        &o,
        &[
            "menos", "less", "fewer", "moins", "weniger", "meno", "kurang", "az ", "少", "덜",
        ],
    );
    let numero = o
        .split(|c: char| !c.is_ascii_digit())
        .find_map(|s| s.parse::<u32>().ok())
        .or_else(|| {
            [
                ("dos", 2),
                ("two", 2),
                ("tres", 3),
                ("three", 3),
                ("cuatro", 4),
                ("four", 4),
                ("cinco", 5),
                ("five", 5),
            ]
            .iter()
            .find(|(p, _)| o.contains(p))
            .map(|(_, n)| *n)
        });
    let columnas = tiene(
        &o,
        &["columna", "column", "colonne", "spalte", "kolom", "sütun"],
    );
    let mut nuevo = b.clone();
    let paso = |n: u32| {
        if mas {
            n + 2
        } else if menos {
            n.saturating_sub(2).max(1)
        } else {
            n
        }
    };
    match &mut nuevo.tipo {
        Tipo::Tabla {
            filas, columnas: c, ..
        } => {
            if columnas {
                *c = numero.unwrap_or_else(|| paso(*c)).clamp(1, 20);
            } else {
                *filas = numero.unwrap_or_else(|| paso(*filas)).clamp(1, 80);
            }
        }
        Tipo::Vinetas { vacias, .. }
        | Tipo::Casillas { vacias, .. }
        | Tipo::Numerada { vacias, .. } => {
            if columnas {
                // Una lista en N columnas: una rejilla con N listas.
                let n = numero.unwrap_or(2).clamp(2, 4);
                let mut hijos = Vec::new();
                for _ in 0..n {
                    let mut h = b.clone();
                    h.id = 0;
                    h.tam = Tam::Auto;
                    hijos.push(h);
                }
                let mut r = hacer::fila(hijos);
                r.tam = b.tam;
                return Some(r);
            }
            *vacias = numero
                .unwrap_or_else(|| paso(*vacias + 1).saturating_sub(1))
                .min(60);
        }
        Tipo::Horario { paso: p, .. } if mas || menos => *p = if mas { 30 } else { 60 },
        Tipo::Rejilla { columnas: c, .. } => *c = numero.unwrap_or_else(|| paso(*c)).clamp(1, 12),
        Tipo::Notas { pauta, .. } | Tipo::Lineas { pauta, .. } => {
            *pauta = if tiene(&o, &["punto", "dot"]) {
                Pauta::Puntos
            } else if tiene(&o, &["cuadr", "grid", "squar"]) {
                Pauta::Cuadricula
            } else if tiene(&o, &["blanc", "blank", "liso", "plain"]) {
                Pauta::Blanco
            } else if tiene(&o, &["raya", "renglon", "line"]) {
                Pauta::Rayado
            } else {
                return None;
            }
        }
        _ if columnas && b.tipo.es_contenedor() => {
            let mut c = b.clone();
            crate::paleta::cambiar_contenedor(
                &mut c,
                if numero.unwrap_or(2) > 2 {
                    "rejilla"
                } else {
                    "fila"
                },
            );
            if let Tipo::Rejilla { columnas: k, .. } = &mut c.tipo {
                *k = numero.unwrap_or(3).clamp(1, 12);
            }
            return Some(c);
        }
        _ => return None,
    }
    (nuevo != *b).then_some(nuevo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 11, 6).unwrap()
    }

    const PLANNER: &str = r#"{"nombre": "Planner semanal", "tema": "planificacion",
      "repeticion": {"cada": "semana", "desde": "2027-01-04", "hasta": "2027-03-28"},
      "raiz": {"tipo": "columna", "hijos": [
        {"tipo": "fila", "hijos": [{"tipo": "encabezado", "texto": "Semana {semana}"}, {"tipo": "campo", "campo": "fecha", "formato": "largo", "alineacion": "derecha"}]},
        {"tipo": "horario", "desde": 7, "hasta": 22, "dias": 7, "tam": "2fr"},
        {"tipo": "fila", "tam": "1fr", "hijos": [
          {"tipo": "seccion", "titulo": "Tareas", "hijos": [{"tipo": "casillas", "vacias": 8}]},
          {"tipo": "seccion", "titulo": "Hábitos", "hijos": [{"tipo": "habitos", "habitos": ["Agua", "Leer"], "dias": 7}]}
        ]}
      ]}}"#;

    #[test]
    fn describir_con_una_respuesta_buena() {
        let c = ClienteFalso::con(&[PLANNER]);
        let p = describir(
            &c,
            "planner semanal con horario de 7 a 22, lista de tareas y hábitos",
            "es",
            hoy(),
        )
        .unwrap();
        assert_eq!(p.doc.nombre, "Planner semanal");
        assert_eq!(p.doc.repeticion.as_ref().unwrap().cada, Cada::Semana);
        assert!(p.avisos.is_empty(), "{:?}", p.avisos);
        let pedido = &c.recibidos.lock().unwrap()[0];
        assert_eq!(pedido.herramienta, HERRAMIENTA_PLANTILLA);
        assert!(pedido.sistema.contains("calendario_mes"));
        assert!(pedido.sistema.contains("«es»"));
        assert!(pedido.esquema["properties"]["raiz"]["properties"]["tipo"]["enum"].is_array());
        // Se maqueta sin problemas: 12 semanas.
        let paginas = crate::exportar::paginas(&p.doc, hoy());
        assert_eq!(paginas.len(), 12);
    }

    #[test]
    fn describir_repara_y_reintenta_una_vez() {
        // La primera no es JSON; la segunda viene en ingles y entre texto.
        let c = ClienteFalso::con(&[
            "Lo siento, no puedo",
            "Sure! ```json\n{\"name\": \"Todo\", \"root\": {\"type\": \"column\", \"children\": [{\"type\": \"checklist\", \"items\": \"pan, leche, huevos\"}, {\"type\": \"sparkles\"}]}}\n```",
        ]);
        let p = describir(&c, "lista de la compra", "es", hoy()).unwrap();
        assert_eq!(c.cuantos(), 2);
        assert!(
            c.recibidos.lock().unwrap()[1]
                .usuario
                .contains("no era JSON")
        );
        match &p.doc.raiz.tipo.hijos().unwrap()[0].tipo {
            Tipo::Casillas { items, .. } => assert_eq!(items.len(), 3),
            otro => panic!("{otro:?}"),
        }
        assert!(p.avisos.iter().any(|a| a.contains("sparkles")));
    }

    #[test]
    fn casos_negativos_del_cliente_y_de_la_respuesta() {
        let c = ClienteFalso::fallando("sin clave");
        assert_eq!(
            describir(&c, "x", "es", hoy()).unwrap_err(),
            ErrorIa::Cliente("sin clave".into())
        );
        let c = ClienteFalso::con(&["nada", "tampoco"]);
        assert!(matches!(
            describir(&c, "x", "es", hoy()),
            Err(ErrorIa::Respuesta(_))
        ));
        let c = ClienteFalso::con(&["{\"tipo\": \"desconocido\"}"]);
        assert!(matches!(
            describir(&c, "x", "es", hoy()),
            Err(ErrorIa::Respuesta(_))
        ));
    }

    #[test]
    fn ajustar_y_sugerir_devuelven_un_bloque() {
        let doc = crate::galeria::por_clave("tareas", &Textos::nuevos("es"), hoy()).unwrap();
        let id = {
            let mut x = 0;
            doc.raiz.recorrer(&mut |b, _| {
                if matches!(b.tipo, Tipo::Casillas { .. }) {
                    x = b.id;
                }
            });
            x
        };
        let c = ClienteFalso::con(&[
            r#"{"type": "checklist", "items": ["Llamar al banco", "Comprar pan"], "vacias": 4}"#,
        ]);
        let (b, _) = sugerir(&c, &doc, id, hoy()).unwrap();
        assert!(matches!(&b.tipo, Tipo::Casillas { items, .. } if items.len() == 2));
        assert!(c.recibidos.lock().unwrap()[0].usuario.contains("casillas"));
        let c = ClienteFalso::con(&[r#"{"raiz": {"tipo": "tabla", "filas": 10, "columnas": 2}}"#]);
        let (b, _) = ajustar(&c, &doc, id, "hazlo tabla", hoy()).unwrap();
        assert!(matches!(b.tipo, Tipo::Tabla { filas: 10, .. }));
        assert!(ajustar(&c, &doc, 99_999, "x", hoy()).is_err());
    }

    #[test]
    fn sin_modelo_entiende_la_frase_del_dueno() {
        let t = Textos::nuevos("es");
        let d = sin_modelo(
            "planner semanal con horario de 7 a 22, lista de tareas y hábitos",
            "es",
            &t,
            hoy(),
        );
        assert_eq!(d.repeticion.as_ref().unwrap().cada, Cada::Semana);
        let mut tipos = Vec::new();
        d.raiz.recorrer(&mut |b, _| tipos.push(b.tipo.clave()));
        assert!(
            tipos.contains(&"horario") && tipos.contains(&"casillas") && tipos.contains(&"habitos")
        );
        let mut horas = None;
        d.raiz.recorrer(&mut |b, _| {
            if let Tipo::Horario {
                desde, hasta, dias, ..
            } = b.tipo
            {
                horas = Some((desde, hasta, dias));
            }
        });
        assert_eq!(horas, Some((7, 22, 7)));
        let m = sin_modelo("calendario mensual con hábitos", "es", &t, hoy());
        assert_eq!(m.repeticion.unwrap().cada, Cada::Mes);
        let diario = sin_modelo("daily journal", "en", &Textos::nuevos("en"), hoy());
        assert_eq!(diario.repeticion.unwrap().cada, Cada::Dia);
        assert_eq!(diario.tema.base, "diario");
    }

    #[test]
    fn ajustar_sin_modelo_mas_filas_y_dos_columnas() {
        let tabla = Bloque::nuevo(Tipo::Tabla {
            filas: 5,
            columnas: 3,
            cabeceras: vec![],
        });
        let mas = ajustar_sin_modelo(&tabla, "más filas").unwrap();
        assert!(matches!(mas.tipo, Tipo::Tabla { filas: 7, .. }));
        let cols = ajustar_sin_modelo(&tabla, "ponla en 4 columnas").unwrap();
        assert!(matches!(cols.tipo, Tipo::Tabla { columnas: 4, .. }));
        let lista = hacer::casillas(&[], 6);
        let dos = ajustar_sin_modelo(&lista, "ponlo en dos columnas").unwrap();
        assert!(matches!(&dos.tipo, Tipo::Fila { hijos, .. } if hijos.len() == 2));
        let notas = hacer::notas("x");
        assert!(matches!(
            ajustar_sin_modelo(&notas, "con puntos").unwrap().tipo,
            Tipo::Notas {
                pauta: Pauta::Puntos,
                ..
            }
        ));
        assert!(ajustar_sin_modelo(&notas, "hazlo bonito").is_none());
    }
}
