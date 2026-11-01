//! **Las recetas del asistente** (13-oct-2026): los pedidos que el usuario
//! repite, como botones encima de la caja del chat «Asistente».
//!
//! Regla de la casa (un solo componente con variables): **una receta es una
//! automatizacion** (`crate::automatizaciones`) con disparador `a_mano` y un
//! solo paso, un pedido al asistente, que ademas lleva `boton: true` y se
//! ensena como boton. Vive en el mismo `automatizaciones.json` del chat
//! «Asistente» y viaja igual (solo con los aparatos propios). Lo que la
//! receta anade va en los campos que la automatizacion guarda tal cual
//! (`resto`):
//!
//! ```json
//! {"id":"auto-1760390000000-0","nombre":"Resumir","activa":true,
//!  "disparador":{"tipo":"a_mano"},
//!  "pasos":[{"tipo":"asistente","pedido":"Resume lo último en 5 puntos"}],
//!  "creada":1760390000000,"tocada":1760390000000,
//!  "boton":true,"icono":"📝","orden":0}
//! ```
//!
//! Sin `aparato`: un boton vale en todos. Pulsarlo manda el pedido como si
//! se hubiera escrito en el chat (no por el motor de automatizaciones: asi
//! la respuesta nace en la conversacion que se esta mirando).

use serde_json::{Value, json};

use crate::automatizaciones::{Automatizacion, Disparador, Entrada, Lista, Paso, id_nuevo};

/// Lo mas largo del nombre y del pedido.
pub const TOPE_NOMBRE: usize = 40;
pub const TOPE_PEDIDO: usize = 2000;
/// Cuantos botones como mucho (es una fila).
pub const MAXIMO: usize = 40;

/// Los iconos que se ofrecen para elegir.
pub const ICONOS: [&str; 12] = [
    "⭐", "📝", "📊", "🎨", "🔎", "📄", "🖼️", "✅", "📅", "💡", "🌐", "🤖",
];

/// Una receta, vista desde su automatizacion.
#[derive(Debug, Clone, PartialEq)]
pub struct Receta {
    pub id: String,
    pub nombre: String,
    pub icono: String,
    pub pedido: String,
    pub orden: i64,
}

/// Lo que puede salir mal al cambiarlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSePuede {
    Vacio,
    Llenas,
    NoEsta,
}

/// Si la automatizacion es una receta (un boton): `a_mano`, encendida, con
/// `boton: true` y un solo paso que es un pedido al asistente.
pub fn de(a: &Automatizacion) -> Option<Receta> {
    if a.disparador != Disparador::AMano
        || !a.activa
        || a.resto.get("boton") != Some(&Value::Bool(true))
    {
        return None;
    }
    let [Paso::Asistente { pedido, .. }] = a.pasos.as_slice() else {
        return None;
    };
    Some(Receta {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        icono: a
            .resto
            .get("icono")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        pedido: pedido.clone(),
        orden: a
            .resto
            .get("orden")
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX),
    })
}

/// Las recetas de la lista, de izquierda a derecha.
pub fn visibles(l: &Lista) -> Vec<Receta> {
    let mut v: Vec<(i64, Receta)> = l
        .buenas()
        .filter_map(|a| de(a).map(|r| (a.creada, r)))
        .collect();
    v.sort_by(|(ca, a), (cb, b)| a.orden.cmp(&b.orden).then(ca.cmp(cb)).then(a.id.cmp(&b.id)));
    v.into_iter().map(|(_, r)| r).collect()
}

fn recortar(t: &str, n: usize) -> String {
    t.replace('\n', " ").trim().chars().take(n).collect()
}

fn buscar_mut<'a>(l: &'a mut Lista, id: &str) -> Option<&'a mut Automatizacion> {
    l.entradas.iter_mut().find_map(|e| match e {
        Entrada::Buena(a) if a.id == id && de(a).is_some() => Some(a),
        _ => None,
    })
}

/// **Una receta nueva**, al final de la fila. Sin nombre, el principio del
/// pedido; sin icono, uno segun lo que pide.
pub fn anadir(
    l: &mut Lista,
    nombre: &str,
    icono: &str,
    pedido: &str,
    ahora: i64,
) -> Result<Receta, NoSePuede> {
    let pedido: String = pedido.trim().chars().take(TOPE_PEDIDO).collect();
    if pedido.is_empty() {
        return Err(NoSePuede::Vacio);
    }
    let fila = visibles(l);
    if fila.len() >= MAXIMO {
        return Err(NoSePuede::Llenas);
    }
    let orden = fila
        .iter()
        .map(|r| r.orden.saturating_add(1))
        .filter(|o| *o != i64::MAX)
        .max()
        .unwrap_or(0);
    let nombre = if nombre.trim().is_empty() {
        nombre_para(&pedido)
    } else {
        recortar(nombre, TOPE_NOMBRE)
    };
    let icono = if icono.trim().is_empty() {
        icono_para(&pedido).to_string()
    } else {
        icono.trim().to_string()
    };
    let mut resto = serde_json::Map::new();
    resto.insert("boton".into(), json!(true));
    resto.insert("icono".into(), json!(icono));
    resto.insert("orden".into(), json!(orden));
    let a = Automatizacion {
        id: id_nuevo(ahora),
        nombre,
        activa: true,
        disparador: Disparador::AMano,
        pasos: vec![Paso::Asistente {
            pedido,
            proyecto: None,
            imagenes: Vec::new(),
        }],
        aparato: None,
        creada: ahora,
        tocada: ahora,
        resto,
    };
    let r = de(&a).ok_or(NoSePuede::NoEsta)?;
    l.anadir(a).map_err(|_| NoSePuede::NoEsta)?;
    Ok(r)
}

/// Cambia una receta (nombre, icono, pedido) y la sella con `ahora`.
pub fn cambiar(
    l: &mut Lista,
    id: &str,
    ahora: i64,
    f: impl FnOnce(&mut Receta),
) -> Result<(), NoSePuede> {
    let a = buscar_mut(l, id).ok_or(NoSePuede::NoEsta)?;
    let mut r = de(a).ok_or(NoSePuede::NoEsta)?;
    f(&mut r);
    let nombre = recortar(&r.nombre, TOPE_NOMBRE);
    let pedido: String = r.pedido.trim().chars().take(TOPE_PEDIDO).collect();
    if pedido.is_empty() || nombre.is_empty() {
        return Err(NoSePuede::Vacio);
    }
    a.nombre = nombre;
    a.pasos = vec![Paso::Asistente {
        pedido,
        proyecto: None,
        imagenes: Vec::new(),
    }];
    a.resto.insert("icono".into(), json!(r.icono.trim()));
    a.resto.insert("orden".into(), json!(r.orden));
    a.tocada = ahora;
    Ok(())
}

/// La quita de la lista (y de las automatizaciones: es la misma).
pub fn borrar(l: &mut Lista, id: &str) -> Result<(), NoSePuede> {
    if buscar_mut(l, id).is_none() {
        return Err(NoSePuede::NoEsta);
    }
    l.borrar(id);
    Ok(())
}

/// La mueve `paso` sitios en la fila (−1: a la izquierda). Renumera la
/// fila de 0 en adelante; solo se sellan las que cambian de sitio.
pub fn mover(l: &mut Lista, id: &str, paso: i32, ahora: i64) -> Result<(), NoSePuede> {
    let mut fila: Vec<String> = visibles(l).into_iter().map(|r| r.id).collect();
    let i = fila.iter().position(|x| x == id).ok_or(NoSePuede::NoEsta)?;
    let j = (i as i64 + paso as i64).clamp(0, fila.len() as i64 - 1) as usize;
    let quien = fila.remove(i);
    fila.insert(j, quien);
    for (n, id) in fila.iter().enumerate() {
        if let Some(a) = buscar_mut(l, id)
            && a.resto.get("orden").and_then(Value::as_i64) != Some(n as i64)
        {
            a.resto.insert("orden".into(), json!(n));
            a.tocada = ahora;
        }
    }
    Ok(())
}

/// El nombre de serie: las primeras palabras del pedido.
pub fn nombre_para(pedido: &str) -> String {
    let mut s = String::new();
    for p in pedido.split_whitespace() {
        if s.chars().count() + p.chars().count() + 1 > 22 {
            break;
        }
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(p.trim_matches(|c: char| !c.is_alphanumeric() && c != '¿' && c != '?'));
    }
    if s.is_empty() {
        s = pedido.chars().take(22).collect();
    }
    s
}

/// El icono de serie, segun lo que se pide.
pub fn icono_para(pedido: &str) -> &'static str {
    let q = pedido.to_lowercase();
    let tiene = |v: &[&str]| v.iter().any(|p| q.contains(p));
    if tiene(&["resum", "apunt", "nota", "escrib", "redact"]) {
        "📝"
    } else if tiene(&["gráfic", "grafic", "barras", "tabla", "datos"]) {
        "📊"
    } else if tiene(&["dibuj", "lienzo", "pinta", "calca"]) {
        "🎨"
    } else if tiene(&["foto", "imagen", "describ", "qué pone", "que pone"]) {
        "🖼️"
    } else if tiene(&["lee", "pdf", "ocr", "texto"]) {
        "📄"
    } else if tiene(&["tarea", "lista", "pendiente"]) {
        "✅"
    } else if tiene(&["traduc", "idioma", "inglés", "ingles"]) {
        "🌐"
    } else {
        "⭐"
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_receta_es_una_automatizacion_a_mano_con_boton() {
        let mut l = Lista::default();
        let r = anadir(&mut l, "", "", "Dibuja un plano de la casa", 10).unwrap();
        assert_eq!(r.icono, "🎨");
        assert_eq!(r.nombre, "Dibuja un plano de la");
        // En el fichero es una automatizacion `a_mano` con sus campos de mas,
        // y vuelve a leerse igual (el formato de las automatizaciones).
        let json = l.a_json();
        assert!(
            json.contains("\"tipo\": \"a_mano\"") && json.contains("\"boton\": true"),
            "{json}"
        );
        assert!(
            json.contains("\"icono\": \"🎨\"") && json.contains("\"orden\": 0"),
            "{json}"
        );
        let otra = Lista::de_json(&json).unwrap();
        assert_eq!(visibles(&otra), vec![r.clone()]);
        assert!(crate::automatizaciones::validar(otra.buscar(&r.id).unwrap()).is_ok());
        // Una automatizacion a mano SIN boton, o apagada, no es receta.
        let mut sin_boton = otra.buscar(&r.id).unwrap().clone();
        sin_boton.resto.remove("boton");
        assert!(de(&sin_boton).is_none());
        let mut apagada = otra.buscar(&r.id).unwrap().clone();
        apagada.activa = false;
        assert!(de(&apagada).is_none());
    }

    #[test]
    fn mover_cambiar_y_borrar() {
        let mut l = Lista::default();
        let a = anadir(&mut l, "A", "", "pedido a", 1).unwrap();
        let b = anadir(&mut l, "B", "📊", "Gráfica de ventas", 2).unwrap();
        let c = anadir(&mut l, "C", "", "Lee este PDF", 3).unwrap();
        assert_eq!(c.icono, "📄");
        mover(&mut l, &c.id, -2, 10).unwrap();
        let fila: Vec<String> = visibles(&l).into_iter().map(|x| x.nombre).collect();
        assert_eq!(fila, ["C", "A", "B"]);
        mover(&mut l, &b.id, 5, 11).unwrap();
        assert_eq!(visibles(&l).last().unwrap().id, b.id);
        cambiar(&mut l, &b.id, 12, |r| r.nombre = "Ventas".into()).unwrap();
        assert_eq!(l.buscar(&b.id).unwrap().tocada, 12);
        assert_eq!(
            visibles(&l).iter().find(|r| r.id == b.id).unwrap().nombre,
            "Ventas"
        );
        // Negativos: vacio, una que no esta, borrar dos veces.
        assert_eq!(anadir(&mut l, "x", "", "   ", 13), Err(NoSePuede::Vacio));
        assert_eq!(
            cambiar(&mut l, &b.id, 14, |r| r.pedido.clear()),
            Err(NoSePuede::Vacio)
        );
        assert_eq!(borrar(&mut l, "no-existe"), Err(NoSePuede::NoEsta));
        borrar(&mut l, &a.id).unwrap();
        assert_eq!(borrar(&mut l, &a.id), Err(NoSePuede::NoEsta));
        assert_eq!(visibles(&l).len(), 2);
        // Tope.
        let mut llenas = Lista::default();
        for i in 0..MAXIMO {
            anadir(&mut llenas, "", "", &format!("p{i}"), i as i64).unwrap();
        }
        assert_eq!(
            anadir(&mut llenas, "", "", "una mas", 99),
            Err(NoSePuede::Llenas)
        );
    }
}
