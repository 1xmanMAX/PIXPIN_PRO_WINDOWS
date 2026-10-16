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
\\text{} para palabras, \\measuredangle para el angulo (∢), \\overset{\\frown}{AC} para el arco AC, \\overline{AB} para un segmento, \\therefore, \\begin{bmatrix}...\\end{bmatrix} (o pmatrix, vmatrix) para matrices y \\begin{cases}...\\end{cases} para sistemas con llave. \
Nada de \\displaystyle, \\left/\\right, \\mathrm, \\operatorname ni otros paquetes. \
Si delante de la formula hay una etiqueta (a), b), 1.), copiala al principio con \\text{a)}.";

const SISTEMA_TIPO: &str = "Clasificas recortes de una hoja escaneada. Responde SOLO una palabra de esta lista: \
texto, titulo, tabla, foto, grafica, dibujo. Si es una formula (tambien un sistema de ecuaciones con su llave, una matriz), responde FORMULA: y detras la formula en LaTeX (sin $; un sistema con \\begin{cases}...\\end{cases}, una matriz con \\begin{bmatrix}...\\end{bmatrix}).";

const SISTEMA_GRAFICA: &str = "Recibes el recorte de una grafica de una hoja. Si es la grafica de una o varias funciones y = f(x), \
responde SOLO un JSON como {\"funciones\":[\"x^2\"],\"x_desde\":-3,\"x_hasta\":3,\"y_desde\":-1,\"y_hasta\":9} \
(formulas en x con ^, sqrt(), sin(), cos(), ln(), e^x; la ventana que se ve). Si no lo es (barras, datos, un dibujo), responde NO.";

/// Pregunta con una imagen y devuelve el texto de la respuesta.
pub fn preguntar(motor: &dyn Motor, sistema: &str, png: Vec<u8>, pregunta: &str) -> Result<String, String> {
    preguntar_con(motor, sistema, png, pregunta, TOKENS)
}

/// Lo mismo con otro tope de tokens (la lista de rotulos de una figura es
/// mas larga que una formula).
pub fn preguntar_con(motor: &dyn Motor, sistema: &str, png: Vec<u8>, pregunta: &str, tokens: u32) -> Result<String, String> {
    let mut p = Peticion::nueva(vec![Mensaje::sistema(sistema), Mensaje::usuario_con_imagen(pregunta, png, "image/png")]);
    p.max_tokens = tokens;
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

const SISTEMA_ROTULOS: &str = "Recibes el recorte de una figura geometrica o un diagrama. Lista TODOS sus rotulos de texto: \
letras de puntos (A, B, O, P...), medidas y angulos (2a, 45°, θ, x), palabras y formulas cortas. \
Responde SOLO un JSON: [{\"t\":\"A\",\"x\":0.12,\"y\":0.40}, ...] con el texto tal cual se ve (griegas en Unicode, grados con °) \
y el centro de cada rotulo en fracciones del ancho y del alto del recorte (0 a 1). \
Despues, en la misma lista, los HECHOS geometricos que se ven o que dice el texto, uno por objeto: \
{\"h\":\"semicircunferencia AB\"}, {\"h\":\"AB es diametro\"}, {\"h\":\"AOB es un cuadrante\"}, {\"h\":\"centro O\"}, {\"h\":\"m∢APB=90°\"} \
(angulos con ∢ y tres letras, vertice en medio). Sin explicaciones.";

const SISTEMA_RICO: &str = "Recibes el recorte de un bloque de texto de una lamina o una hoja (puede llevar formulas). \
Transcribelo EXACTO, renglon a renglon, en este formato de texto rico: \
formulas en linea entre $...$ con LaTeX (\\sphericalangle para el angulo ∢, \\overline{AB} para la raya, \\overset{\\frown}{AC} para el arco, \\frac{a}{b}, 90^\\circ, \\therefore); \
*cursiva*, **negrita**, <u>subrayado</u>; el color de un tramo que no es negro con \\color{rojo}{...} (rojo, verde, azul, naranja, morado, gris o #rrggbb), tambien dentro de una formula; \
un renglon centrado empieza con \\centro (sin llaves, seguido del texto); las vinetas con •. \
MUY IMPORTANTE: los colores. Mira el color de cada letra: lo rojo va con \\color{rojo}{...}, lo verde con \\color{verde}{...}, lo azul con \\color{azul}{...} (dentro de una formula tambien: $X=90^\\circ - \\color{rojo}{(a+b)}$). \
Los renglones que en el papel van centrados o sangrados bajo una vineta (las formulas de debajo) empiezan con \\centro. \
Fuera de $...$ no uses otros mandatos (ni \\text ni \\boxed: los recuadros ya estan dibujados). \
Copia las letras tal cual (con sus tildes), sin corregir ni resumir. \
Responde SOLO el texto, sin explicaciones ni bloques de codigo.";

/// **Un bloque de texto en texto rico del lienzo** (`pixpin_motor2d::texto_rico`):
/// la columna «RESOLUCION» de una lamina con sus formulas en linea, sus
/// colores por tramo, su cursiva y su subrayado. El OCR de Windows leia
/// «Äb» por «AB» con su raya y «m<COD» por «m∢COD».
pub fn pedir_rico(motor: &dyn Motor, png: Vec<u8>, pista: &str) -> Result<Leido, String> {
    // Con lo que el OCR vio de color (Haiku se olvidaba de los colores).
    let pregunta = if pista.is_empty() { "Transcribe este bloque.".to_string() } else { format!("Transcribe este bloque. Colores que se ven (del OCR, puede leer mal las letras): {pista}") };
    let t = preguntar_con(motor, SISTEMA_RICO, png, &pregunta, 3000)?;
    leer_rico(&t).map(Leido::Rico).ok_or_else(|| format!("el modelo no dio el texto: «{}»", corto(&t)))
}

/// Lee la transcripcion: sin bloque de codigo, sin barras dobles de mas,
/// sin renglones vacios al principio y al final. `None` si no hay nada.
pub fn leer_rico(t: &str) -> Option<String> {
    let t = sin_bloque(t);
    let t = pixpin_motor2d::texto_rico::sin_barras_dobles(&t);
    let mut t = t.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ').replace("\r\n", "\n");
    // Un renglon en blanco entre parrafos se queda (es el aire del papel);
    // mas de uno, no.
    while t.contains("\n\n\n") {
        t = t.replace("\n\n\n", "\n\n");
    }
    // Un renglon con LaTeX y sin `$` («θ=90^\circ»): es una formula entera.
    // Y un `$` que no se cerro (la respuesta cortada), se cierra.
    let t = t
        .split('\n')
        .map(|l| {
            let (pre, resto) = match l.strip_prefix("\\centro") {
                Some(r) => ("\\centro ", r.trim_start()),
                None => ("", l),
            };
            let n = resto.matches('$').count();
            // (No si lleva palabras: «Grafica de f(x) = x^2» como formula entera
            // perdia sus espacios.)
            let sin_mandatos: String = {
                let mut s = String::new();
                let mut c = resto.chars().peekable();
                while let Some(ch) = c.next() {
                    if ch == '\\' {
                        while c.peek().is_some_and(|x| x.is_ascii_alphabetic()) {
                            c.next();
                        }
                        s.push(' ');
                    } else {
                        s.push(ch);
                    }
                }
                s
            };
            let con_palabras = sin_mandatos.split(|c: char| !c.is_alphabetic()).any(|p| p.chars().count() >= 4);
            if n == 0 && !con_palabras && (resto.contains('\\') || resto.contains('^')) {
                format!("{pre}${resto}$")
            } else if n == 0 && (resto.contains('\\') || resto.contains('^')) {
                // Con palabras: solo los trozos de formula («x^2»).
                let t: Vec<String> = resto.split(' ').map(|p| if p.contains('\\') || p.contains('^') { format!("${p}$") } else { p.to_string() }).collect();
                format!("{pre}{}", t.join(" "))
            } else if n % 2 == 1 {
                format!("{pre}{resto}$")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!t.is_empty() && !t.eq_ignore_ascii_case("nada")).then_some(t)
}

#[cfg(test)]
mod pruebas_rico {
    #[test]
    fn el_texto_rico_de_la_nube_se_arregla() {
        assert_eq!(super::leer_rico("θ=90^\\circ").unwrap(), "$θ=90^\\circ$");
        assert_eq!(super::leer_rico("\\centro X=90^\\circ").unwrap(), "\\centro $X=90^\\circ$");
        assert_eq!(super::leer_rico("Por ∢ interior:\n$45^\\circ=\\frac{2a").unwrap(), "Por ∢ interior:\n$45^\\circ=\\frac{2a$");
        assert_eq!(super::leer_rico("```\nNos piden $X$\n\n\n\nY\n```").unwrap(), "Nos piden $X$\n\nY");
        assert!(super::leer_rico("NADA").is_none());
        assert_eq!(super::leer_rico("Grafica de f(x) = x^2 - 2").unwrap(), "Grafica de f(x) = $x^2$ - 2");
    }
}

/// **Los rotulos de una figura** (letras de los puntos, medidas, angulos)
/// con su centro en fracciones del recorte: la nube corrige al OCR en lo
/// que lee mal o no ve (una letra suelta, una griega).
pub fn pedir_rotulos(motor: &dyn Motor, png: Vec<u8>) -> Result<Leido, String> {
    let t = preguntar_con(motor, SISTEMA_ROTULOS, png, "Lista los rotulos de esta figura.", 1500)?;
    let hechos = leer_hechos(&t);
    leer_rotulos(&t).map(|r| Leido::Rotulos(r, hechos)).ok_or_else(|| format!("el modelo no dio los rotulos: «{}»", corto(&t)))
}

/// Los hechos de la lista (`{"h":"AB es diametro"}`), tal cual.
pub fn leer_hechos(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut resto = t;
    while let Some(a) = resto.find('{') {
        let Some(b) = resto[a..].find('}') else { break };
        if let Ok(r) = serde_json::from_str::<Value>(&resto[a..a + b + 1])
            && let Some(h) = r.get("h").and_then(Value::as_str)
            && !h.trim().is_empty()
        {
            v.push(h.trim().to_string());
        }
        resto = &resto[a + b + 1..];
    }
    v
}

/// Lee la lista de rotulos (`[{"t":..,"x":..,"y":..}]`, con o sin bloque).
pub fn leer_rotulos(t: &str) -> Option<Vec<(String, f64, f64)>> {
    let t = sin_bloque(t);
    // Objeto a objeto: una lista cortada por el tope de tokens vale hasta
    // donde llego (banco del 15-oct: la de la lamina llegaba a medias).
    let mut lista = Vec::new();
    let mut resto = t.as_str();
    let mut alguno = false;
    while let Some(a) = resto.find('{') {
        let Some(b) = resto[a..].find('}') else { break };
        alguno = true;
        if let Ok(r) = serde_json::from_str::<Value>(&resto[a..a + b + 1]) {
            let texto = r.get("t").or_else(|| r.get("texto")).and_then(Value::as_str).map(|s| s.trim().to_string());
            let x = r.get("x").and_then(Value::as_f64);
            let y = r.get("y").and_then(Value::as_f64);
            if let (Some(texto), Some(x), Some(y)) = (texto, x, y)
                && !texto.is_empty()
                && (0.0..=1.0).contains(&x)
                && (0.0..=1.0).contains(&y)
            {
                lista.push((texto, x, y));
            }
        }
        resto = &resto[a + b + 1..];
    }
    (alguno || t.contains('[')).then_some(lista)
}

#[cfg(test)]
mod pruebas_rotulos {
    use super::*;

    #[test]
    fn los_rotulos_se_leen_aunque_lleguen_cortados() {
        let t = "```json\n[{\"t\":\"A\",\"x\":0.27,\"y\":0.12},{\"t\":\"2a\",\"x\":0.37,\"y\":0.04},{\"t\":\"B\",\"x\":0.8";
        let v = leer_rotulos(t).unwrap();
        assert_eq!(v, vec![("A".to_string(), 0.27, 0.12), ("2a".to_string(), 0.37, 0.04)]);
        assert_eq!(leer_rotulos("[]"), Some(Vec::new()));
        assert!(leer_rotulos("No veo rotulos").is_none());
        // Fuera de 0-1: no.
        assert!(leer_rotulos("[{\"t\":\"A\",\"x\":3,\"y\":0.1}]").unwrap().is_empty());
    }
}
