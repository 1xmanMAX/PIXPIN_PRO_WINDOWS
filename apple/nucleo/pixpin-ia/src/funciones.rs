//! **Graficar funciones** (`graficar_funcion`): `y = f(x)`, una o varias,
//! con su dominio, como curvas sobre el **instrumento plano cartesiano** del
//! lienzo (`pixpin_motor2d::instrumento`), y su ecuacion compuesta al lado
//! (`pixpin_motor2d::ecuacion`), del color de su curva.
//!
//! **Las cuentas las hace Rust, no el modelo**: el modelo solo escribe la
//! formula (`x^2 - 3sin(x)`) y el dominio; el interprete del lienzo
//! (`pixpin_motor2d::formula`, el mismo de la grafica del panel y del movil)
//! la evalua en cientos de puntos. Un modelo que calcula puntos a mano se
//! equivoca, y uno de 2B mas.
//!
//! El instrumento tiene una sola unidad para los dos ejes y el origen en su
//! centro (`instrumento::punto`): se elige la unidad para que quepan el
//! dominio y los valores, y la curva se corta donde la funcion no existe o
//! se sale (asi `1/x` deja su hueco y `tan(x)` sale por ramas).

use serde_json::{Map, Value, json};

use pixpin_motor2d::ecuacion::{self, Estilo};
use pixpin_motor2d::elemento::{ColorRgba, Elemento};
use pixpin_motor2d::formula::{self, Compilada};
use pixpin_motor2d::instrumento as ins;
use pixpin_motor2d::vector::Punto2;

use crate::herramientas::{ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;

pub const NOMBRE: &str = "graficar_funcion";
/// Cuantas funciones como mucho en una grafica.
pub const MAX_FUNCIONES: usize = 6;
/// Lo mas lejos que llega un limite.
pub const LIMITE: f64 = 1e6;
/// Cuantas muestras de `x`.
pub const MUESTRAS: usize = 600;
/// La mayor diferencia de escala entre el alto y el ancho de la ventana.
const PROPORCION: f64 = 4.0;

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Grafica funciones y = f(x) en el lienzo, como curvas sobre un plano cartesiano, con su ecuacion al lado. ",
            "Las calcula la app: tu solo pon las formulas (x^2, sin(x), 2x+1, sqrt(x), abs(x), ln(x), e^x; por partes: x^2 si x<0; x si x>=0) ",
            "y el dominio. Ejemplo: {\"funciones\":[\"x^2\",\"2x+1\"],\"x_desde\":-3,\"x_hasta\":3}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["funciones"],
            "properties": {
                "funciones": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_FUNCIONES, "description": "formulas en x"},
                "x_desde": {"type": "number"},
                "x_hasta": {"type": "number"},
                "y_desde": {"type": "number", "description": "omitir: se ajusta a la curva"},
                "y_hasta": {"type": "number"},
                "titulo": {"type": "string"}
            }
        }),
    }
}

/// Lo que se pide, ya comprobado.
#[derive(Debug, Clone, PartialEq)]
pub struct Funciones {
    /// Las formulas tal como se escribieron (para el rotulo) y compiladas.
    pub formulas: Vec<String>,
    pub x_desde: f64,
    pub x_hasta: f64,
    pub y_desde: f64,
    pub y_hasta: f64,
    pub titulo: String,
}

fn numero(args: &Map<String, Value>, k: &str) -> Result<Option<f64>, ErrorHerramienta> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_f64().filter(|v| v.is_finite()).map(Some).ok_or_else(|| mal(format!("«{k}» no es un numero"))),
        Some(Value::String(s)) => s
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or_else(|| mal(format!("«{k}» = «{s}» no es un numero"))),
        Some(_) => Err(mal(format!("«{k}» tiene que ser un numero"))),
    }
}

pub fn validar(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Funciones, ErrorHerramienta> {
    let lista: Vec<String> = match args.get("funciones").or(args.get("funcion")).or(args.get("formula")) {
        Some(Value::String(s)) => s.split(['\n', '|']).map(str::to_string).collect(),
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        _ => return Err(mal("falta «funciones» (una lista de formulas en x, como \"x^2\")")),
    };
    let lista: Vec<String> = lista.into_iter().map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
    if lista.is_empty() {
        return Err(mal("«funciones» esta vacia"));
    }
    if lista.len() > MAX_FUNCIONES {
        return Err(mal(format!("como mucho {MAX_FUNCIONES} funciones por grafica")));
    }
    let mut compiladas = Vec::new();
    for f in &lista {
        if f.chars().count() > 160 {
            return Err(mal(format!("la formula «{}...» es demasiado larga", f.chars().take(30).collect::<String>())));
        }
        compiladas.push(compilar(f)?);
    }
    let (x_desde, x_hasta) = match (numero(args, "x_desde")?, numero(args, "x_hasta")?) {
        (None, None) => (-5.0, 5.0),
        (Some(a), Some(b)) => (a, b),
        _ => return Err(mal("pon los dos limites del dominio: «x_desde» y «x_hasta»")),
    };
    // `partial_cmp`: un NaN tambien es un dominio al reves.
    if x_desde.partial_cmp(&x_hasta) != Some(std::cmp::Ordering::Less) {
        return Err(mal(format!("el dominio esta al reves o vacio: x_desde = {x_desde} tiene que ser menor que x_hasta = {x_hasta}")));
    }
    if x_desde.abs() > LIMITE || x_hasta.abs() > LIMITE || x_hasta - x_desde < 1e-9 {
        return Err(mal(format!("el dominio tiene que estar entre -{LIMITE} y {LIMITE} y no ser un punto")));
    }
    // Las muestras, para ver si hay curva y ajustar la ventana de la y.
    let mut ys: Vec<f64> = Vec::new();
    for (f, c) in lista.iter().zip(&compiladas) {
        let antes = ys.len();
        for i in 0..=MUESTRAS {
            let x = x_desde + (x_hasta - x_desde) * i as f64 / MUESTRAS as f64;
            let y = c.en(x);
            if y.is_finite() && y.abs() <= LIMITE {
                ys.push(y);
            }
        }
        if ys.len() - antes < 2 {
            return Err(mal(format!("«{f}» no tiene valores reales entre x = {x_desde} y x = {x_hasta}: cambia el dominio")));
        }
    }
    let (y_desde, y_hasta) = match (numero(args, "y_desde")?, numero(args, "y_hasta")?) {
        (Some(a), Some(b)) if a < b && a.abs() <= LIMITE && b.abs() <= LIMITE => (a, b),
        (Some(a), Some(b)) => return Err(mal(format!("la ventana de y esta al reves o fuera de rango: y_desde = {a}, y_hasta = {b}"))),
        (None, None) => ventana(&mut ys),
        (Some(a), None) => (a, ventana(&mut ys).1.max(a + 1.0)),
        (None, Some(b)) => (ventana(&mut ys).0.min(b - 1.0), b),
    };
    // Una sola unidad para los dos ejes: si la y se dispara, la curva se
    // corta arriba y abajo en vez de dejar la x en un hilo.
    let ax = x_desde.abs().max(x_hasta.abs());
    let ay = y_desde.abs().max(y_hasta.abs());
    let (y_desde, y_hasta) = if ay > ax * PROPORCION {
        avisos.push(format!(
            "la curva sube mucho mas de lo que mide el dominio: se ensena y entre {} y {} (pide y_desde/y_hasta para otra ventana)",
            ins::escrito(y_desde.max(-ax * PROPORCION)),
            ins::escrito(y_hasta.min(ax * PROPORCION))
        ));
        (y_desde.max(-ax * PROPORCION), y_hasta.min(ax * PROPORCION))
    } else {
        (y_desde, y_hasta)
    };
    let titulo = args.get("titulo").and_then(Value::as_str).unwrap_or("").trim().chars().take(120).collect();
    Ok(Funciones { formulas: lista, x_desde, x_hasta, y_desde, y_hasta, titulo })
}

/// Una formula en `x`, o el motivo. Acepta `y = ...` y `f(x) = ...`.
fn compilar(f: &str) -> Result<Compilada, ErrorHerramienta> {
    let c = formula::compilar(f).ok_or_else(|| mal(format!("la formula «{f}» no se entiende; escribela como x^2 + 3x - 1 o sin(x)/x")))?;
    if let Some(v) = c.variables().into_iter().find(|v| *v != 'x') {
        return Err(mal(format!("«{f}» usa la variable «{v}»; solo vale x")));
    }
    Ok(c)
}

/// La ventana de la y que deja ver la curva: lo que hay entre los
/// percentiles 2 y 98 (un polo como el de `1/x` no aplasta lo demas), con
/// un margen, y el cero dentro si cae cerca.
fn ventana(ys: &mut [f64]) -> (f64, f64) {
    ys.sort_by(f64::total_cmp);
    let n = ys.len();
    let (mut a, mut b) = (ys[n * 2 / 100], ys[(n * 98 / 100).min(n - 1)]);
    if b - a < 1e-9 {
        a -= 1.0;
        b += 1.0;
    }
    let margen = (b - a) * 0.08;
    let (mut a, mut b) = (a - margen, b + margen);
    if a > 0.0 && a < (b - a) * 0.5 {
        a = 0.0;
    }
    if b < 0.0 && -b < (b - a) * 0.5 {
        b = 0.0;
    }
    (a, b)
}

/// Los colores de las curvas (los de la grafica del lienzo).
fn color_de_curva(k: usize) -> ColorRgba {
    pixpin_motor2d::grafica::color_de(pixpin_motor2d::grafica::COLORES_DE_CURVAS[k % pixpin_motor2d::grafica::COLORES_DE_CURVAS.len()])
}

/// **Los elementos**: el instrumento, las curvas por ramas, la leyenda con
/// la ecuacion de cada una y el titulo, todo en un grupo.
pub(crate) fn elementos(f: &mut Fabrica, p: &Funciones) {
    let piezas = piezas(p, (60.0, 40.0, 680.0, 520.0));
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("funciones"));
    f.de_motor(&piezas);
    f.grupo = antes;
}

/// Las piezas del motor dentro de la caja `(x, y, ancho, alto)` del lienzo
/// de 800x600.
pub fn piezas(p: &Funciones, (cx, cy, cw, ch): (f64, f64, f64, f64)) -> Vec<Elemento> {
    let negro = ColorRgba::opaco(0.12, 0.12, 0.12);
    let familia = "Fraunces";
    let medir = |t: &str, tam: f32| crate::lienzo::medir_con(t, tam, familia);
    let estilo = Estilo { color: negro, tam: 18.0, opacidad: 1.0, familia: familia.into() };
    let compiladas: Vec<Compilada> = p.formulas.iter().filter_map(|f| formula::compilar(f)).collect();
    // La leyenda, para saber cuanto sitio deja.
    let cajas: Vec<ecuacion::Caja> = compiladas
        .iter()
        .map(|c| ecuacion::caja_de_la_ecuacion("y", c, estilo.tam, &medir))
        .collect();
    let ancho_leyenda = cajas.iter().map(|c| c.ancho as f64).fold(0.0, f64::max).min(cw * 0.4);
    let alto_titulo = if p.titulo.is_empty() { 0.0 } else { 40.0 };
    let (w, h) = (cw - ancho_leyenda - 24.0, ch - alto_titulo);
    let ax = p.x_desde.abs().max(p.x_hasta.abs());
    let ay = p.y_desde.abs().max(p.y_hasta.abs());
    // La unidad: que quepan el dominio y la ventana de la y.
    let u = (w / (2.0 * ax)).min(h / (2.0 * ay)).clamp(0.05, 400.0);
    let (lado_x, lado_y) = (2.0 * ax * u, 2.0 * ay * u);
    let mut e = ins::elemento_de_fabrica(2, negro, 1.5, 1.0);
    if let pixpin_motor2d::elemento::Figura::Instrumento(i) = &mut e.figura {
        i.unidad = Some(u);
        // Numeros cada 1, 2, 5, 10...: ni amontonados ni sueltos.
        let paso = pixpin_motor2d::grafica::paso_bonito(2.0 * ax.max(ay)).max(ins::paso_util(1.0, u, 34.0).min(1e9));
        i.paso_numeros = Some(paso);
        i.paso_cuadros = Some(paso / if paso >= 2.0 { 2.0 } else { 1.0 });
    }
    e.ancho = lado_x as f32;
    e.alto = lado_y as f32;
    e.x = (cx + (w - lado_x) / 2.0) as f32;
    e.y = (cy + alto_titulo + (h - lado_y) / 2.0) as f32;
    let i = *ins::de(&e).expect("es un instrumento");
    let mut salida = vec![e.clone()];
    for (k, c) in compiladas.iter().enumerate() {
        let de_curva = estilo.con_color(color_de_curva(k));
        let mut rama: Vec<Punto2> = Vec::new();
        let cerrar = |rama: &mut Vec<Punto2>, salida: &mut Vec<Elemento>| {
            if rama.len() >= 2 {
                salida.push(ecuacion::elemento_linea(std::mem::take(rama), 2.5, &de_curva));
            }
            rama.clear();
        };
        let mut anterior: Option<f64> = None;
        for s in 0..=MUESTRAS {
            let x = p.x_desde + (p.x_hasta - p.x_desde) * s as f64 / MUESTRAS as f64;
            let y = c.en(x);
            // Un salto enorme entre dos muestras es un polo: se corta.
            let salto = anterior.is_some_and(|a| (y - a).abs() > (p.y_hasta - p.y_desde) * 0.9);
            if !y.is_finite() || y < p.y_desde || y > p.y_hasta || salto {
                cerrar(&mut rama, &mut salida);
                anterior = y.is_finite().then_some(y);
                if !y.is_finite() || y < p.y_desde || y > p.y_hasta {
                    continue;
                }
            }
            anterior = Some(y);
            rama.push(ins::punto(&e, &i, x, y, 0.0));
        }
        cerrar(&mut rama, &mut salida);
    }
    // La leyenda, a la derecha, del color de cada curva.
    let mut y_ley = e.y + 8.0;
    let x_ley = e.x + e.ancho + 24.0;
    for (k, caja) in cajas.iter().enumerate() {
        salida.extend(caja.elementos(x_ley, y_ley, &estilo.con_color(color_de_curva(k))));
        y_ley += caja.alto + 12.0;
    }
    if !p.titulo.is_empty() {
        let tam = 26.0;
        let (aw, ah) = medir(&p.titulo, tam);
        salida.push(ecuacion::elemento_texto(
            &p.titulo,
            (cx + cw / 2.0) as f32 - aw / 2.0,
            cy as f32,
            (aw, ah),
            tam,
            false,
            &estilo,
        ));
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn validar_(v: Value) -> Result<Funciones, ErrorHerramienta> {
        let Value::Object(m) = v else { panic!() };
        validar(&m, &mut Vec::new())
    }

    #[test]
    fn dos_curvas_sobre_el_instrumento() {
        let p = validar_(json!({"funciones": ["x^2", "y = 2x + 1"], "x_desde": -3, "x_hasta": 3})).unwrap();
        assert_eq!(p.formulas.len(), 2);
        let els = piezas(&p, (60.0, 40.0, 680.0, 520.0));
        assert!(matches!(els[0].figura, pixpin_motor2d::elemento::Figura::Instrumento(_)));
        let curvas: Vec<_> = els
            .iter()
            .filter(|e| matches!(&e.figura, pixpin_motor2d::elemento::Figura::Linea { puntos } if puntos.len() > 50))
            .collect();
        assert_eq!(curvas.len(), 2);
        // El (2, 4) de la parabola cae donde el instrumento dice.
        let i = *ins::de(&els[0]).unwrap();
        let p24 = ins::punto(&els[0], &i, 2.0, 4.0, 0.0);
        let pasa = match &curvas[0].figura {
            pixpin_motor2d::elemento::Figura::Linea { puntos } => puntos.iter().any(|q| (q.x - p24.x).abs() < 2.0 && (q.y - p24.y).abs() < 2.0),
            _ => false,
        };
        assert!(pasa, "la curva no pasa por (2, 4)");
        // Lo lee el lienzo, todo editable y en un grupo.
        let mut f = Fabrica::nueva((0.0, 0.0), 5);
        elementos(&mut f, &p);
        let l = pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        assert_eq!(f.salida[0]["type"], "pixpin-axes");
        let leidos = l.elementos();
        assert!(leidos.iter().all(|e| e.grupos.last() == leidos[0].grupos.last() && !e.grupos.is_empty()));
    }

    #[test]
    fn un_polo_corta_la_curva() {
        let p = validar_(json!({"funciones": ["1/x"], "x_desde": -4, "x_hasta": 4})).unwrap();
        let els = piezas(&p, (0.0, 0.0, 800.0, 600.0));
        let ramas = els.iter().filter(|e| matches!(&e.figura, pixpin_motor2d::elemento::Figura::Linea { puntos } if puntos.len() > 20)).count();
        assert_eq!(ramas, 2, "1/x son dos ramas");
    }

    #[test]
    fn dominios_y_formulas_malas_se_rechazan() {
        for (v, motivo) in [
            (json!({"funciones": ["x^2"], "x_desde": 5, "x_hasta": -5}), "al reves"),
            (json!({"funciones": ["x^2"], "x_desde": 1, "x_hasta": 1}), "al reves"),
            (json!({"funciones": ["x^2"], "x_desde": 0}), "los dos limites"),
            (json!({"funciones": ["x^2"], "x_desde": -1e9, "x_hasta": 1}), "entre"),
            (json!({"funciones": ["sqrt(x)"], "x_desde": -5, "x_hasta": -1}), "valores reales"),
            (json!({"funciones": ["x +* )"]}), "no se entiende"),
            (json!({"funciones": ["x + t"]}), "variable"),
            (json!({"funciones": ["x + z"]}), "no se entiende"),
            (json!({"funciones": []}), "vacia"),
            (json!({}), "falta"),
            (json!({"funciones": ["x"], "y_desde": 3, "y_hasta": 1}), "al reves"),
            (json!({"funciones": ["x", "x", "x", "x", "x", "x", "x"]}), "como mucho"),
            (json!({"funciones": ["x"], "x_desde": "mucho", "x_hasta": 1}), "no es un numero"),
        ] {
            let e = validar_(v.clone()).unwrap_err().to_string();
            assert!(e.contains(motivo), "{v}: {e}");
        }
    }

    #[test]
    fn la_ventana_se_ajusta_y_no_deja_la_x_en_un_hilo() {
        let mut avisos = Vec::new();
        let Value::Object(m) = json!({"funciones": ["x^3"], "x_desde": -10, "x_hasta": 10}) else { panic!() };
        let p = validar(&m, &mut avisos).unwrap();
        assert!(p.y_hasta <= 40.0 + 1e-9, "{}", p.y_hasta);
        assert_eq!(avisos.len(), 1);
        let p = validar_(json!({"funciones": ["sin(x)"], "x_desde": -6.3, "x_hasta": 6.3})).unwrap();
        assert!(p.y_desde < -0.9 && p.y_hasta > 0.9 && p.y_hasta < 2.0);
    }
}
