//! **Lo que se le pregunta al modelo con vision**, siempre con SOLO el
//! recorte de una zona (pocos tokens, respuesta corta) y sin herramientas:
//!
//! - una formula → su LaTeX ([`pedir_latex`]);
//! - una zona dudosa → que es ([`pedir_tipo`]);
//! - una grafica → si es de funciones, cuales y su ventana
//!   ([`pedir_grafica`], que luego dibuja `graficar_funcion`).
//!
//! Las respuestas se leen con manga ancha (con o sin `$`, con bloque de
//! codigo, con una frase delante) y lo que no se entiende es un error que la
//! app convierte en el camino sin modelo (la formula como imagen).

use serde_json::{Map, Value};

use super::Tipo;
use super::montaje::Leido;
use crate::motor::{Mensaje, Motor, Peticion, Seguir};

/// Lo mas que se deja contestar (una formula larga cabe de sobra).
pub const TOKENS: u32 = 400;

const SISTEMA_LATEX: &str = "Eres un transcriptor de formulas. Recibes el recorte de una formula de una hoja escaneada. \
Responde SOLO con la formula en LaTeX, sin $ ni explicaciones ni bloques de codigo. \
Si tiene varios renglones, separalos con \\\\. Si no hay ninguna formula, responde NADA. \
Usa SOLO este LaTeX: \\frac{}{}, \\sqrt{} y \\sqrt[n]{}, ^ y _, \\int, \\sum, \\lim, \\infty, griegas, \\cdot, \\pm, \\le, \\ge, \\neq, \\to, \\sin \\cos \\tan \\ln \\log, \
\\text{} para palabras, \\begin{bmatrix}...\\end{bmatrix} (o pmatrix, vmatrix) para matrices y \\begin{cases}...\\end{cases} para sistemas con llave. \
Nada de \\displaystyle, \\left/\\right, \\mathrm, \\operatorname ni otros paquetes. \
Si delante de la formula hay una etiqueta (a), b), 1.), copiala al principio con \\text{a)}.";

const SISTEMA_TIPO: &str = "Clasificas recortes de una hoja escaneada. Responde SOLO una palabra de esta lista: \
texto, titulo, tabla, foto, grafica, dibujo. Si es una formula (tambien un sistema de ecuaciones con su llave, una matriz), responde FORMULA: y detras la formula en LaTeX (sin $; un sistema con \\begin{cases}...\\end{cases}, una matriz con \\begin{bmatrix}...\\end{bmatrix}).";

const SISTEMA_GRAFICA: &str = "Recibes el recorte de una grafica de una hoja. Si es la grafica de una o varias funciones y = f(x), \
responde SOLO un JSON como {\"funciones\":[\"x^2\"],\"x_desde\":-3,\"x_hasta\":3,\"y_desde\":-1,\"y_hasta\":9} \
(formulas en x con ^, sqrt(), sin(), cos(), ln(), e^x; la ventana que se ve). Si no lo es (barras, datos, un dibujo), responde NO.";

/// Pregunta con una imagen y devuelve el texto de la respuesta.
pub fn preguntar(motor: &dyn Motor, sistema: &str, png: Vec<u8>, pregunta: &str) -> Result<String, String> {
    let mut p = Peticion::nueva(vec![Mensaje::sistema(sistema), Mensaje::usuario_con_imagen(pregunta, png, "image/png")]);
    p.max_tokens = TOKENS;
    p.temperatura = 0.0;
    let r = motor.completar(&p, &mut |_| Seguir::Si).map_err(|e| e.to_string())?;
    Ok(r.texto.trim().to_string())
}

/// **La formula del recorte, en LaTeX** (una linea por renglon).
pub fn pedir_latex(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar(motor, SISTEMA_LATEX, png, "Transcribe esta formula a LaTeX.")?;
    leer_latex(&t).map(Leido::Latex).ok_or_else(|| format!("el modelo no dio una formula: «{}»", corto(&t)))
}

/// **Que es una zona dudosa.**
pub fn pedir_tipo(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar(motor, SISTEMA_TIPO, png, "¿Que es este recorte?")?;
    leer_dudosa(&t).ok_or_else(|| format!("el modelo no dijo que es: «{}»", corto(&t)))
}

/// Lee lo que dijo de una zona dudosa: un tipo, o `FORMULA: <latex>` (de
/// una vez, sin otra pregunta).
pub fn leer_dudosa(t: &str) -> Option<Leido> {
    let limpio = sin_bloque(t);
    let s = limpio.trim();
    let cabeza: String = s.chars().take(8).collect::<String>().to_lowercase();
    if cabeza.starts_with("formula") || cabeza.starts_with("fórmula") {
        if let Some((_, latex)) = s.split_once(':')
            && let Some(l) = leer_latex(latex)
        {
            return Some(Leido::Latex(l));
        }
        return Some(Leido::Tipo(Tipo::Formula));
    }
    // Con una frase delante: la primera palabra de la lista que salga.
    leer_tipo(s).or_else(|| s.split(|c: char| !c.is_alphanumeric()).find_map(leer_tipo)).map(Leido::Tipo)
}

/// **Si una grafica es de funciones**: sus argumentos de `graficar_funcion`,
/// o `None` si no lo es.
pub fn pedir_grafica(motor: &dyn Motor, png: Vec<u8>) -> Result<Option<Leido>, String> {
    let t = preguntar(motor, SISTEMA_GRAFICA, png, "¿Es la grafica de funciones?")?;
    Ok(leer_grafica(&t).map(Leido::Funciones))
}

fn corto(t: &str) -> String {
    t.chars().take(80).collect()
}

/// Quita un bloque de codigo de alrededor (```latex ... ```).
fn sin_bloque(t: &str) -> String {
    let t = t.trim();
    if let Some(r) = t.strip_prefix("```") {
        let r = r.split_once('\n').map_or(r, |(_, resto)| resto);
        return r.trim_end().trim_end_matches("```").trim().to_string();
    }
    t.to_string()
}

/// **Lee el LaTeX** de la respuesta: sin `$`, sin `\[ \]`, sin bloque de
/// codigo; renglones por `\\` o salto de linea. `None` si no hay formula.
pub fn leer_latex(t: &str) -> Option<Vec<String>> {
    let t = sin_bloque(t);
    let t = t.trim().trim_matches('$').trim();
    let t = t.trim_start_matches("\\[").trim_end_matches("\\]").trim();
    let t = t.trim_start_matches("\\(").trim_end_matches("\\)").trim();
    if t.is_empty() || t.eq_ignore_ascii_case("nada") || t.len() > crate::ecuacion::MAX_LARGO {
        return None;
    }
    // Una frase de mas («Aqui esta: ...»): lo de despues de los dos puntos.
    let t = match t.split_once(": ") {
        Some((antes, despues)) if antes.split_whitespace().count() >= 2 && !antes.contains('\\') && !antes.contains('=') => despues,
        _ => t,
    };
    // Los renglones, sin partir las matrices (`\\` dentro de un entorno es
    // una fila suya, no un renglon).
    let lineas: Vec<String> = crate::ecuacion::partir_lineas(t)
        .into_iter()
        .map(|l| l.trim_matches('$').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    (!lineas.is_empty()).then_some(lineas)
}

/// Lee el tipo de una palabra (con tildes, mayusculas o punto).
pub fn leer_tipo(t: &str) -> Option<Tipo> {
    let q = crate::acciones::normalizar(t);
    let p = q.split(|c: char| !c.is_alphanumeric()).find(|p| !p.is_empty())?;
    Some(match p {
        "texto" | "parrafo" => Tipo::Parrafo,
        "titulo" | "encabezado" => Tipo::Encabezado,
        "formula" | "ecuacion" => Tipo::Formula,
        "tabla" => Tipo::Tabla,
        "foto" | "fotografia" | "imagen" => Tipo::Foto,
        "grafica" | "grafico" => Tipo::Grafica,
        "dibujo" | "diagrama" | "esquema" => Tipo::Dibujo,
        _ => return None,
    })
}

/// Lee el JSON de una grafica de funciones (con o sin bloque de codigo).
/// `None` si dice que no o no trae funciones.
pub fn leer_grafica(t: &str) -> Option<Map<String, Value>> {
    let t = sin_bloque(t);
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    let v: Value = serde_json::from_str(t.get(a..=b)?).ok()?;
    let m = v.as_object()?.clone();
    let n = m.get("funciones").and_then(Value::as_array).map_or(0, Vec::len);
    (n > 0).then_some(m)
}
