//! **La biblioteca de figuras desde el asistente** (`figuras_guardadas`):
//! «guarda esto como arbol isometrico», «pon el arbol isometrico», «¿que
//! figuras tengo?».
//!
//! Es la MISMA biblioteca del panel de figuras del lienzo
//! (`pixpin_motor2d::biblioteca`, fichero `figuras.json` en el formato del
//! movil): lo que guarda el asistente sale en el menu del lienzo y al
//! reves. Esta herramienta solo dice que se pide; la ejecuta la app, que
//! sabe donde vive el fichero y que hay elegido en el lienzo abierto.

use serde_json::{Map, Value, json};

use pixpin_motor2d::biblioteca::FiguraGuardada;

use crate::herramientas::{ErrorHerramienta, Herramienta};

pub const NOMBRE: &str = "figuras_guardadas";

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: format!(
            "{}{}{}{}",
            "La biblioteca de figuras del lienzo (las del usuario y las plantillas de serie). «accion»: guardar (lo elegido en el lienzo, o si no ",
            "lo ultimo que dibujaste, con el «nombre» que diga el usuario o uno corto que le pongas tu), poner (estampa la figura de ese «nombre») ",
            "o lista (las que hay). «alargar»: rehace la PLANTILLA elegida en el lienzo (o la de «nombre») con otro intervalo, con el mismo diseño: ",
            format_args!(
                "«desde»/«hasta» (del 1 al 40: una hoja mas larga), «de_a» (de 10 en 10: una hoja por cada 10), «semanas_desde»/«semanas_hasta» (cronograma), \
                 «mes» AAAA-MM y «meses» (planificador mensual y habitos con el calendario de verdad). Se alargan: {}. \
                 Ejemplos: {{\"accion\":\"poner\",\"nombre\":\"arbol isometrico\"}}, {{\"accion\":\"alargar\",\"desde\":1,\"hasta\":40,\"de_a\":10}}",
                pixpin_motor2d::figuras_de_serie::las_que_se_alargan()
            )
        ),
        parametros: json!({
            "type": "object",
            "required": ["accion"],
            "properties": {
                "accion": {"type": "string", "enum": ["guardar", "poner", "lista", "alargar"]},
                "nombre": {"type": "string"},
                "desde": {"type": "integer"},
                "hasta": {"type": "integer"},
                "de_a": {"type": "integer"},
                "semanas_desde": {"type": "integer"},
                "semanas_hasta": {"type": "integer"},
                "mes": {"type": "string"},
                "meses": {"type": "integer"}
            }
        }),
    }
}

/// Lo que se pide a la biblioteca.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoFiguras {
    Guardar { nombre: String },
    Poner { nombre: String },
    Lista,
    /// **Una plantilla con otro intervalo** (22-oct): la elegida en el
    /// lienzo, o la de `nombre`.
    Alargar { nombre: Option<String>, intervalo: pixpin_motor2d::figuras_de_serie::Intervalo },
}

/// Un entero del pedido (el modelo a veces lo manda como texto).
fn entero(args: &Map<String, Value>, k: &str) -> Result<Option<u32>, ErrorHerramienta> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .or_else(|| v.as_f64().filter(|f| *f >= 0.0 && f.fract() == 0.0).map(|f| f as u64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| mal(format!("«{k}» tiene que ser un numero entero"))),
    }
}

/// «2026-11», «11/2026» o «noviembre 2026».
fn mes_de(s: &str) -> Option<(i32, u32)> {
    const MESES: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];
    let s = crate::acciones::normalizar(s.trim());
    let partes: Vec<&str> = s.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty() && *p != "de" && *p != "del").collect();
    let [a, b] = partes[..] else { return None };
    let numero = |p: &str| p.parse::<i64>().ok();
    let mes = |p: &str| numero(p).filter(|m| (1..=12).contains(m)).map(|m| m as u32).or_else(|| MESES.iter().position(|m| p.starts_with(m)).map(|i| i as u32 + 1));
    match (numero(a), numero(b)) {
        (Some(x), _) if x > 31 => Some((x as i32, mes(b)?)),
        _ => Some((numero(b).filter(|x| *x > 31)? as i32, mes(a)?)),
    }
}

fn intervalo(args: &Map<String, Value>) -> Result<pixpin_motor2d::figuras_de_serie::Intervalo, ErrorHerramienta> {
    let semanas = match (entero(args, "semanas_desde")?, entero(args, "semanas_hasta")?) {
        (None, None) => None,
        (d, h) => Some((d.unwrap_or(1), h.or(d).unwrap_or(12))),
    };
    let mes = match args.get("mes").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(s) => Some(mes_de(s).ok_or_else(|| mal(format!("«mes» = «{s}»: escribelo como AAAA-MM (2026-11)")))?),
        None => None,
    };
    Ok(pixpin_motor2d::figuras_de_serie::Intervalo {
        desde: entero(args, "desde")?,
        hasta: entero(args, "hasta")?,
        de_a: entero(args, "de_a")?,
        semanas,
        mes,
        meses: entero(args, "meses")?,
    })
}

pub fn validar(args: &Map<String, Value>) -> Result<PedidoFiguras, ErrorHerramienta> {
    let nombre = ["nombre", "figura", "como"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .map(|s| s.trim().trim_matches(['«', '»', '"', '\'']).trim().chars().take(80).collect::<String>())
        .filter(|s| !s.is_empty());
    match args.get("accion").and_then(Value::as_str).map(crate::acciones::normalizar).as_deref() {
        Some("guardar" | "guarda" | "salvar" | "save") => Ok(PedidoFiguras::Guardar {
            nombre: nombre.ok_or_else(|| mal("para guardar hace falta el «nombre» de la figura"))?,
        }),
        Some("poner" | "pon" | "insertar" | "estampar" | "usar" | "put" | "insert") => Ok(PedidoFiguras::Poner {
            nombre: nombre.ok_or_else(|| mal("falta el «nombre» de la figura que poner"))?,
        }),
        Some("lista" | "listar" | "ver" | "cuales" | "list") => Ok(PedidoFiguras::Lista),
        Some("alargar" | "intervalo" | "ampliar" | "extender" | "continuar" | "dividir" | "partir") => {
            Ok(PedidoFiguras::Alargar { nombre, intervalo: intervalo(args)? })
        }
        None if nombre.is_none() => Ok(PedidoFiguras::Lista),
        Some(otra) => Err(mal(format!("«accion» = «{otra}» no existe; usa guardar, poner, lista o alargar"))),
        None => Err(mal("falta «accion»: guardar, poner, lista o alargar")),
    }
}

/// **La figura que mejor encaja con el nombre pedido**: igual sin tildes ni
/// mayusculas; si no, la que contiene todas las palabras pedidas; si no, la
/// que mas comparte. `None` si no se parece ninguna.
pub fn buscar<'a>(figuras: &'a [FiguraGuardada], pedido: &str) -> Option<&'a FiguraGuardada> {
    let q = crate::acciones::normalizar(pedido.trim());
    let palabras = |s: &str| -> Vec<String> {
        crate::acciones::normalizar(s)
            .split(|c: char| !c.is_alphanumeric())
            .filter(|p| p.len() > 2 || p.chars().all(|c| c.is_ascii_digit()))
            // El plural no cuenta: «arboles» es «arbol».
            .map(|p| match p.strip_suffix("es").filter(|r| r.len() > 3) {
                Some(r) => r.to_string(),
                None => p.strip_suffix('s').unwrap_or(p).to_string(),
            })
            .filter(|p| !p.is_empty())
            .collect()
    };
    if let Some(f) = figuras.iter().find(|f| crate::acciones::normalizar(&f.nombre) == q) {
        return Some(f);
    }
    let pq = palabras(pedido);
    if pq.is_empty() {
        return None;
    }
    let comun = |f: &FiguraGuardada| {
        let pf = palabras(&f.nombre);
        pq.iter().filter(|p| pf.contains(p)).count()
    };
    if let Some(f) = figuras.iter().find(|f| comun(f) == pq.len()) {
        return Some(f);
    }
    figuras.iter().filter(|f| comun(f) > 0).max_by_key(|f| comun(f))
}

/// La lista para el modelo (y el usuario).
pub fn lista(figuras: &[FiguraGuardada]) -> String {
    if figuras.is_empty() {
        return "no hay figuras guardadas todavia".into();
    }
    let nombres: Vec<&str> = figuras.iter().map(|f| f.nombre.as_str()).collect();
    format!("{} figuras guardadas: {}", figuras.len(), nombres.join(", "))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::Elemento;

    fn f(nombre: &str) -> FiguraGuardada {
        FiguraGuardada { id: nombre.into(), nombre: nombre.into(), elementos: vec![Elemento::default()] }
    }

    #[test]
    fn se_entienden_las_tres_acciones() {
        let v = |j: Value| validar(j.as_object().unwrap());
        assert_eq!(v(json!({"accion": "guardar", "nombre": "«Árbol isométrico»"})).unwrap(), PedidoFiguras::Guardar { nombre: "Árbol isométrico".into() });
        assert_eq!(v(json!({"accion": "pon", "nombre": "arbol"})).unwrap(), PedidoFiguras::Poner { nombre: "arbol".into() });
        assert_eq!(v(json!({"accion": "lista"})).unwrap(), PedidoFiguras::Lista);
        assert_eq!(v(json!({})).unwrap(), PedidoFiguras::Lista);
        // Casos negativos.
        assert!(v(json!({"accion": "guardar"})).is_err());
        assert!(v(json!({"accion": "poner", "nombre": "  "})).is_err());
        assert!(v(json!({"accion": "borrar_todo", "nombre": "x"})).is_err());
        assert!(v(json!({"nombre": "x"})).is_err());
    }

    #[test]
    fn busca_por_nombre_aunque_no_sea_exacto() {
        let l = vec![f("Árbol isométrico"), f("Casa"), f("Arbol de navidad"), f("Norte")];
        assert_eq!(buscar(&l, "arbol isometrico").unwrap().nombre, "Árbol isométrico");
        assert_eq!(buscar(&l, "el árbol isométrico").unwrap().nombre, "Árbol isométrico");
        assert_eq!(buscar(&l, "CASA").unwrap().nombre, "Casa");
        assert_eq!(buscar(&l, "arboles de navidad").unwrap().nombre, "Arbol de navidad");
        assert!(buscar(&l, "perro").is_none());
        assert!(buscar(&[], "casa").is_none());
        assert!(lista(&l).contains("4 figuras"));
        assert!(lista(&[]).contains("no hay"));
    }

    #[test]
    fn se_entiende_alargar_una_plantilla() {
        use pixpin_motor2d::figuras_de_serie::Intervalo;
        let v = |j: Value| validar(j.as_object().unwrap());
        assert_eq!(
            v(json!({"accion": "alargar", "desde": 1, "hasta": "40", "de_a": 10.0})).unwrap(),
            PedidoFiguras::Alargar { nombre: None, intervalo: Intervalo { desde: Some(1), hasta: Some(40), de_a: Some(10), ..Default::default() } }
        );
        let PedidoFiguras::Alargar { nombre, intervalo } =
            v(json!({"accion": "alargar", "nombre": "cronograma", "semanas_desde": 13, "semanas_hasta": 24})).unwrap()
        else {
            panic!()
        };
        assert_eq!(nombre.as_deref(), Some("cronograma"));
        assert_eq!(intervalo.semanas, Some((13, 24)));
        for (s, esperado) in [("2026-11", (2026, 11)), ("11/2026", (2026, 11)), ("noviembre 2026", (2026, 11)), ("Enero de 2027", (2027, 1))] {
            let PedidoFiguras::Alargar { intervalo, .. } = v(json!({"accion": "alargar", "mes": s, "meses": 3})).unwrap() else { panic!() };
            assert_eq!(intervalo.mes, Some(esperado), "{s}");
            assert_eq!(intervalo.meses, Some(3));
        }
        assert!(v(json!({"accion": "alargar", "mes": "pronto"})).is_err());
        assert!(v(json!({"accion": "alargar", "desde": -3})).is_err());
        assert!(herramienta().descripcion.contains("Cronograma"));
    }
}
