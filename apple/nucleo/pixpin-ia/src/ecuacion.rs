//! **Ecuaciones compuestas en el lienzo** (`escribir_ecuacion`): la formula
//! como se escribe en la pizarra —fracciones con su raya, raices con su
//! tejado, exponentes subidos, griegas— y no `sqrt(b^2-4ac)` a maquina.
//!
//! No hay un compositor nuevo: se juntan los dos que ya hay.
//!
//! - **Leer**: `pixpin_docs::formulas::leer`, el de las formulas de las
//!   notas (el `Formulas.kt` de Android): entiende el LaTeX de bolsillo
//!   (`\frac`, `\sqrt[3]{}`, `\pm`, griegas, `x^2_i`) y la escritura corriente
//!   (`1/2`, `sqrt(x)`, `pi`, `<=`). Lo que no entiende sale tal cual.
//! - **Componer**: las cajas de `pixpin_motor2d::ecuacion` (las de la leyenda
//!   de la grafica del lienzo), que dan textos y rayas del lienzo: se mueven,
//!   se recolorean y salen igual en el PDF, el SVG y el movil.

use serde_json::{Map, Value, json};

use pixpin_motor2d::ecuacion::{self as caja, Caja, Estilo};
use pixpin_motor2d::elemento::{ColorRgba, Elemento};
use pixpin_docs::formulas::{self, Pieza};

use crate::herramientas::{ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;

pub const NOMBRE: &str = "escribir_ecuacion";
/// Lo mas larga que se acepta una ecuacion (todas sus lineas).
pub const MAX_LARGO: usize = 600;
/// Cuantas lineas como mucho.
pub const MAX_LINEAS: usize = 12;
/// La letra de las ecuaciones: con serifa, como en un libro.
pub const FAMILIA: &str = "Fraunces";

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Escribe en el lienzo una ecuacion bien compuesta (fracciones, raices, exponentes, subindices, griegas), en LaTeX ",
            "o escrita normal. Varias lineas: separalas con \\\\ o con saltos de linea. ",
            "Ejemplo: {\"ecuacion\":\"x = \\\\frac{-b \\\\pm \\\\sqrt{b^2 - 4ac}}{2a}\"}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["ecuacion"],
            "properties": {
                "ecuacion": {"type": "string", "maxLength": MAX_LARGO},
                "x": {"type": "number"},
                "y": {"type": "number"},
                "tam_letra": {"type": "number", "description": "por omision 32"},
                "color": {"type": "string"}
            }
        }),
    }
}

/// Lo que se pide.
#[derive(Debug, Clone, PartialEq)]
pub struct Ecuacion {
    pub lineas: Vec<String>,
    /// La esquina de arriba a la izquierda; `None`: centrada en el lienzo.
    pub donde: Option<(f64, f64)>,
    pub tam: f64,
    pub color: String,
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Ecuacion, ErrorHerramienta> {
    let texto = ["ecuacion", "formula", "latex", "texto"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| mal("falta «ecuacion» (en LaTeX o escrita normal)"))?;
    if texto.chars().count() > MAX_LARGO {
        return Err(mal(format!("la ecuacion pasa de {MAX_LARGO} caracteres: partela en varias")));
    }
    // Los modelos escapan de mas (`\\frac` en vez de `\frac`, medido con
    // Haiku el 14-oct): una barra doble pegada a una letra es un mandato,
    // no un salto de linea.
    let mut sin_dobles = String::with_capacity(texto.len());
    let car: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < car.len() {
        if car[i] == '\\' && car.get(i + 1) == Some(&'\\') && car.get(i + 2).is_some_and(|c| c.is_ascii_alphabetic()) {
            i += 1;
            continue;
        }
        sin_dobles.push(car[i]);
        i += 1;
    }
    let texto = sin_dobles.as_str();
    // Sin los `$` de alrededor ni los entornos de alineado.
    let limpio = texto
        .trim_matches('$')
        .replace("\\begin{aligned}", "")
        .replace("\\end{aligned}", "")
        .replace("\\[", "")
        .replace("\\]", "")
        .replace('&', "");
    let lineas: Vec<String> = limpio
        .split("\\\\")
        .flat_map(|l| l.split('\n'))
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    if lineas.is_empty() {
        return Err(mal("la ecuacion esta vacia"));
    }
    if lineas.len() > MAX_LINEAS {
        return Err(mal(format!("como mucho {MAX_LINEAS} lineas por llamada")));
    }
    // Llaves descuadradas: casi siempre una llamada cortada.
    let abre = limpio.matches('{').count();
    let cierra = limpio.matches('}').count();
    if abre != cierra {
        return Err(mal(format!("las llaves no cuadran ({abre} «{{» y {cierra} «}}»)")));
    }
    let raros: Vec<String> = lineas
        .iter()
        .flat_map(|l| desconocidos(&formulas::leer(l)))
        .collect();
    if !raros.is_empty() {
        avisos.push(format!("no se entendio {} y sale tal cual", raros.join(", ")));
    }
    let num = |k: &str, r: (f64, f64)| -> Result<Option<f64>, ErrorHerramienta> {
        match args.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite() && (r.0..=r.1).contains(n))
                .map(Some)
                .ok_or_else(|| mal(format!("«{k}» tiene que ser un numero de {} a {}", r.0, r.1))),
        }
    };
    let donde = match (num("x", (-200.0, 2000.0))?, num("y", (-200.0, 2000.0))?) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let tam = num("tam_letra", (8.0, 160.0))?.unwrap_or(32.0);
    let color = args
        .get("color")
        .and_then(Value::as_str)
        .and_then(crate::herramientas::color)
        .map_or_else(|| "#1e1e1e".to_string(), |c| c.0);
    Ok(Ecuacion { lineas, donde, tam, color })
}

/// Los mandatos que el lector no entendio (salen con su barra).
fn desconocidos(p: &Pieza) -> Vec<String> {
    match p {
        Pieza::Texto { texto, .. } if texto.starts_with('\\') && texto.len() > 1 => vec![texto.clone()],
        Pieza::Texto { .. } => Vec::new(),
        Pieza::Fila(l) => l.iter().flat_map(desconocidos).collect(),
        Pieza::Fraccion(a, b) => [desconocidos(a), desconocidos(b)].concat(),
        Pieza::ConIndices { base, arriba, abajo } => {
            let mut v = desconocidos(base);
            v.extend(arriba.iter().flat_map(|a| desconocidos(a)));
            v.extend(abajo.iter().flat_map(|a| desconocidos(a)));
            v
        }
        Pieza::Raiz { dentro, indice } => {
            let mut v = desconocidos(dentro);
            v.extend(indice.iter().flat_map(|a| desconocidos(a)));
            v
        }
        Pieza::Agrupado { dentro, .. } => desconocidos(dentro),
    }
}

/// **Del arbol de la formula a la caja compuesta**, con las cajas del motor.
pub fn componer(p: &Pieza, t: f32, medir: caja::Medir<'_>) -> Caja {
    match p {
        Pieza::Texto { texto, cursiva } => {
            if texto.is_empty() {
                caja::hueco(0.0)
            } else {
                caja::letra(texto, t, *cursiva, medir)
            }
        }
        Pieza::Fila(l) => caja::fila(l.iter().map(|p| componer(p, t, medir)).collect()),
        Pieza::Fraccion(a, b) => {
            // Una fraccion dentro de otra, mas pequena (como en un libro).
            let menor = caja::menor(t).max(t * 0.85);
            caja::fraccion(componer(a, menor, medir), componer(b, menor, medir), t)
        }
        Pieza::ConIndices { base, arriba, abajo } => {
            let mut c = componer(base, t, medir);
            if let Some(b) = abajo {
                c = caja::subindice(c, componer(b, caja::menor(t), medir), t);
            }
            if let Some(a) = arriba {
                c = caja::potencia(c, componer(a, caja::menor(t), medir), t);
            }
            c
        }
        Pieza::Raiz { dentro, indice } => caja::raiz(
            componer(dentro, t, medir),
            indice.as_ref().map(|i| componer(i, caja::menor(caja::menor(t)), medir)),
            t,
        ),
        Pieza::Agrupado { dentro, abre, cierra } => {
            let d = componer(dentro, t, medir);
            match abre.as_str() {
                "(" => caja::entre_parentesis(d, t, medir),
                "|" => caja::entre_barras(d, t),
                _ => caja::fila(vec![caja::letra(abre, t, false, medir), d, caja::letra(cierra, t, false, medir)]),
            }
        }
    }
}

/// Las piezas del motor de la ecuacion, ya colocadas. Devuelve tambien lo
/// que ocupa.
pub fn piezas(e: &Ecuacion) -> (Vec<Elemento>, (f32, f32)) {
    let medir = |t: &str, tam: f32| crate::lienzo::medir_con(t, tam, FAMILIA);
    let color = pixpin_motor2d::excalidraw::color_desde(Some(&json!(e.color))).unwrap_or(ColorRgba::opaco(0.12, 0.12, 0.12));
    let estilo = Estilo { color, tam: e.tam as f32, opacidad: 1.0, familia: FAMILIA.into() };
    let cajas: Vec<Caja> = e
        .lineas
        .iter()
        .map(|l| componer(&formulas::leer(l), e.tam as f32, &medir))
        .collect();
    let col = caja::columna(cajas, e.tam as f32 * 0.6);
    let (x, y) = match e.donde {
        Some((x, y)) => (x as f32, y as f32),
        None => (
            (crate::herramientas::ANCHO_LIENZO as f32 - col.ancho) / 2.0,
            (crate::herramientas::ALTO_LIENZO as f32 - col.alto) / 2.0,
        ),
    };
    (col.elementos(x, y, &estilo), (col.ancho, col.alto))
}

pub(crate) fn elementos(f: &mut Fabrica, e: &Ecuacion) {
    let (piezas, _) = piezas(e);
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("ecuacion"));
    f.de_motor(&piezas);
    f.grupo = antes;
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::Figura;

    fn validar_(v: Value) -> Result<Ecuacion, ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        validar(&m, &mut Vec::new())
    }

    #[test]
    fn la_formula_general_sale_compuesta() {
        let e = validar_(json!({"ecuacion": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}"})).unwrap();
        let (els, (w, h)) = piezas(&e);
        let textos: Vec<String> = els
            .iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        let rayas = els.iter().filter(|e| matches!(e.figura, Figura::Linea { .. })).count();
        // La raya de la fraccion y el signo de la raiz.
        assert!(rayas >= 2, "{rayas}");
        assert!(textos.iter().any(|t| t.contains('±')), "{textos:?}");
        assert!(textos.iter().any(|t| t == "2"), "el exponente: {textos:?}");
        // Alta: lleva una fraccion.
        assert!(h > e.tam as f32 * 1.8, "{w}x{h}");
        let mut f = Fabrica::nueva((0.0, 0.0), 2);
        elementos(&mut f, &e);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
    }

    #[test]
    fn el_latex_escapado_de_mas_sigue_siendo_una_linea() {
        // Lo que mando Haiku: `\\frac` con dos barras.
        let e = validar_(json!({"ecuacion": r"x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}"})).unwrap();
        assert_eq!(e.lineas.len(), 1, "{:?}", e.lineas);
        let (els, _) = piezas(&e);
        assert!(els.iter().filter(|e| matches!(e.figura, Figura::Linea { .. })).count() >= 2);
    }

    #[test]
    fn varias_lineas_y_escritura_corriente() {
        let e = validar_(json!({"ecuacion": "E = mc^2 \\\\ a^2 + b^2 = c^2\nsqrt(2)/2", "x": 10, "y": 20})).unwrap();
        assert_eq!(e.lineas.len(), 3);
        let (els, _) = piezas(&e);
        let y0 = els.iter().map(|e| e.y).fold(f32::MAX, f32::min);
        assert!((y0 - 20.0).abs() < 30.0);
    }

    #[test]
    fn lo_roto_se_rechaza() {
        assert!(validar_(json!({})).is_err());
        assert!(validar_(json!({"ecuacion": "   "})).is_err());
        assert!(validar_(json!({"ecuacion": "\\frac{a}{b"})).unwrap_err().to_string().contains("llaves"));
        assert!(validar_(json!({"ecuacion": "x".repeat(MAX_LARGO + 1)})).is_err());
        assert!(validar_(json!({"ecuacion": "x", "tam_letra": 900})).is_err());
        // Un mandato desconocido no rompe: sale tal cual, con aviso.
        let mut avisos = Vec::new();
        let Value::Object(m) = json!({"ecuacion": "\\inventado{x} + 1"}) else { panic!() };
        assert!(validar(&m, &mut avisos).is_ok());
        assert_eq!(avisos.len(), 1, "{avisos:?}");
    }
}
