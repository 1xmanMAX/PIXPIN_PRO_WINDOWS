//! Formato condicional basico: comparar con valores (`cellIs`), buscar texto,
//! los primeros/ultimos N, por encima/debajo del promedio, escalas de color
//! y barras de datos. Las reglas con formula libre (`expression`) no se
//! evaluan: no hay motor de formulas.

use crate::estilos::{Dxf, Rgb, Tema, color_de};
use crate::hoja::{Celda, Hoja, Rango, Valor};
use crate::xml::Nodo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCelda {
    Entre,
    NoEntre,
    Igual,
    Distinto,
    Mayor,
    MayorIgual,
    Menor,
    MenorIgual,
}

/// Un valor de una regla: numero o texto.
#[derive(Debug, Clone, PartialEq)]
pub enum Dato {
    Numero(f64),
    Texto(String),
}

/// Un punto de una escala o barra (`cfvo`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Punto {
    Minimo,
    Maximo,
    Numero(f64),
    Porcentaje(f64),
    Percentil(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Regla {
    Celda {
        op: OpCelda,
        datos: Vec<Dato>,
        dxf: Option<u32>,
    },
    /// 0 contiene, 1 no contiene, 2 empieza por, 3 acaba en.
    Texto {
        modo: u8,
        texto: String,
        dxf: Option<u32>,
    },
    Primeros {
        rango: u32,
        porcentaje: bool,
        ultimos: bool,
        dxf: Option<u32>,
    },
    Promedio {
        encima: bool,
        igual: bool,
        dxf: Option<u32>,
    },
    Escala {
        puntos: Vec<(Punto, Rgb)>,
    },
    Barra {
        min: Punto,
        max: Punto,
        color: Rgb,
        ver_valor: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Estadistica {
    pub min: f64,
    pub max: f64,
    pub media: f64,
    /// Los numeros ordenados (para percentiles y primeros N).
    pub ordenados: Vec<f64>,
}

/// Un bloque `<conditionalFormatting sqref>` con sus reglas por prioridad.
#[derive(Debug, Clone, PartialEq)]
pub struct Condicional {
    pub rangos: Vec<Rango>,
    pub reglas: Vec<(i32, Regla, bool)>,
    pub estadistica: Option<Estadistica>,
}

/// Lo que el formato condicional hace a una celda.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Efecto {
    pub dxf: Dxf,
    /// Color de una escala (relleno).
    pub escala: Option<Rgb>,
    /// Barra: fraccion de la celda (0-1) y color.
    pub barra: Option<(f32, Rgb)>,
    /// La barra pide esconder el valor.
    pub ocultar_valor: bool,
}

fn dato(s: &str) -> Option<Dato> {
    let s = s.trim();
    if let Some(t) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Some(Dato::Texto(t.replace("\"\"", "\"")));
    }
    s.parse::<f64>().ok().map(Dato::Numero)
}

fn punto(n: &Nodo) -> Punto {
    let val = n.attr("val").and_then(|v| v.trim().parse::<f64>().ok());
    match n.attr("type").unwrap_or("") {
        "min" => Punto::Minimo,
        "max" => Punto::Maximo,
        "percent" => Punto::Porcentaje(val.unwrap_or(0.0)),
        "percentile" => Punto::Percentil(val.unwrap_or(50.0)),
        _ => val.map(Punto::Numero).unwrap_or(Punto::Minimo),
    }
}

/// Lee los `<conditionalFormatting>` de la cola de una hoja.
pub fn leer(raiz: &Nodo, tema: &Tema, paleta: &[u32]) -> Vec<Condicional> {
    let mut v = Vec::new();
    let mut bloques = Vec::new();
    raiz.todos("conditionalFormatting", &mut bloques);
    for b in bloques {
        let rangos: Vec<Rango> = b.attr("sqref").unwrap_or("").split_whitespace().filter_map(Rango::leer).collect();
        if rangos.is_empty() {
            continue;
        }
        let mut reglas = Vec::new();
        for r in b.hijos_de("cfRule") {
            let prioridad = r.attr("priority").and_then(|p| p.parse().ok()).unwrap_or(i32::MAX);
            let parar = r.attr("stopIfTrue").is_some_and(|v| v == "1");
            let dxf = r.attr("dxfId").and_then(|d| d.parse().ok());
            let formulas: Vec<String> = r.hijos_de("formula").map(|f| f.texto.clone()).collect();
            let regla = match r.attr("type").unwrap_or("") {
                "cellIs" => {
                    let op = match r.attr("operator").unwrap_or("") {
                        "between" => OpCelda::Entre,
                        "notBetween" => OpCelda::NoEntre,
                        "equal" => OpCelda::Igual,
                        "notEqual" => OpCelda::Distinto,
                        "greaterThan" => OpCelda::Mayor,
                        "greaterThanOrEqual" => OpCelda::MayorIgual,
                        "lessThan" => OpCelda::Menor,
                        "lessThanOrEqual" => OpCelda::MenorIgual,
                        _ => continue,
                    };
                    let datos: Vec<Dato> = formulas.iter().filter_map(|f| dato(f)).collect();
                    if datos.is_empty() {
                        continue;
                    }
                    Regla::Celda { op, datos, dxf }
                }
                t @ ("containsText" | "notContainsText" | "beginsWith" | "endsWith") => {
                    let Some(texto) = r.attr("text") else {
                        continue;
                    };
                    let modo = match t {
                        "containsText" => 0,
                        "notContainsText" => 1,
                        "beginsWith" => 2,
                        _ => 3,
                    };
                    Regla::Texto { modo, texto: texto.to_lowercase(), dxf }
                }
                "top10" => Regla::Primeros {
                    rango: r.attr("rank").and_then(|v| v.parse().ok()).unwrap_or(10),
                    porcentaje: r.attr("percent").is_some_and(|v| v == "1"),
                    ultimos: r.attr("bottom").is_some_and(|v| v == "1"),
                    dxf,
                },
                "aboveAverage" => Regla::Promedio {
                    encima: r.attr("aboveAverage").is_none_or(|v| v != "0"),
                    igual: r.attr("equalAverage").is_some_and(|v| v == "1"),
                    dxf,
                },
                "colorScale" => {
                    let Some(e) = r.hijo("colorScale") else {
                        continue;
                    };
                    let ps: Vec<Punto> = e.hijos_de("cfvo").map(punto).collect();
                    let cs: Vec<Rgb> = e.hijos_de("color").map(|c| color_de(c, tema, paleta).unwrap_or(Rgb::BLANCO)).collect();
                    if ps.len() < 2 || ps.len() != cs.len() {
                        continue;
                    }
                    Regla::Escala { puntos: ps.into_iter().zip(cs).collect() }
                }
                "dataBar" => {
                    let Some(e) = r.hijo("dataBar") else {
                        continue;
                    };
                    let ps: Vec<Punto> = e.hijos_de("cfvo").map(punto).collect();
                    let color = e.hijo("color").and_then(|c| color_de(c, tema, paleta)).unwrap_or(Rgb { r: 0x63, g: 0x8E, b: 0xC6 });
                    Regla::Barra {
                        min: ps.first().copied().unwrap_or(Punto::Minimo),
                        max: ps.get(1).copied().unwrap_or(Punto::Maximo),
                        color,
                        ver_valor: e.attr("showValue").is_none_or(|v| v != "0"),
                    }
                }
                _ => continue,
            };
            reglas.push((prioridad, regla, parar));
        }
        if reglas.is_empty() {
            continue;
        }
        reglas.sort_by_key(|r| r.0);
        v.push(Condicional { rangos, reglas, estadistica: None });
    }
    v
}

impl Condicional {
    pub fn aplica(&self, fila: u32, col: u32) -> bool {
        self.rangos.iter().any(|r| r.contiene(fila, col))
    }

    /// Calcula minimo, maximo, media y orden de los numeros de sus rangos.
    pub fn calcular(&mut self, hoja: &Hoja) {
        let mut nums = Vec::new();
        for r in &self.rangos {
            for f in hoja.filas_entre(r.fila0, r.fila1) {
                for c in hoja.celdas_de(f) {
                    if c.col >= r.col0 && c.col <= r.col1 {
                        if let Valor::Numero(v) = c.valor {
                            nums.push(v);
                        }
                    }
                }
            }
        }
        if nums.is_empty() {
            self.estadistica = Some(Estadistica::default());
            return;
        }
        nums.sort_by(|a, b| a.total_cmp(b));
        let media = nums.iter().sum::<f64>() / nums.len() as f64;
        self.estadistica = Some(Estadistica { min: nums[0], max: nums[nums.len() - 1], media, ordenados: nums });
    }

    fn valor_de(&self, p: Punto, e: &Estadistica) -> f64 {
        match p {
            Punto::Minimo => e.min,
            Punto::Maximo => e.max,
            Punto::Numero(n) => n,
            Punto::Porcentaje(q) => e.min + (e.max - e.min) * q / 100.0,
            Punto::Percentil(q) => {
                if e.ordenados.is_empty() {
                    return 0.0;
                }
                let pos = (q / 100.0).clamp(0.0, 1.0) * (e.ordenados.len() - 1) as f64;
                let i = pos.floor() as usize;
                let j = (i + 1).min(e.ordenados.len() - 1);
                e.ordenados[i] + (e.ordenados[j] - e.ordenados[i]) * (pos - i as f64)
            }
        }
    }

    /// Lo que las reglas de este bloque hacen a la celda, sobre `ef`.
    /// Devuelve `true` si una regla pidio parar (`stopIfTrue`).
    pub fn evaluar(&self, celda: &Celda, texto: &str, dxfs: &[Dxf], ef: &mut Efecto) -> bool {
        let Some(e) = &self.estadistica else {
            return false;
        };
        let num = match celda.valor {
            Valor::Numero(v) => Some(v),
            _ => None,
        };
        for (_, regla, parar) in &self.reglas {
            let (cumple, dxf) = match regla {
                Regla::Celda { op, datos, dxf } => {
                    let cmp = |d: &Dato| -> Option<std::cmp::Ordering> {
                        match (d, num) {
                            (Dato::Numero(x), Some(v)) => v.partial_cmp(x),
                            (Dato::Texto(t), None) => Some(texto.to_lowercase().cmp(&t.to_lowercase())),
                            _ => None,
                        }
                    };
                    use std::cmp::Ordering::*;
                    let a = datos.first().and_then(cmp);
                    let b = datos.get(1).and_then(cmp);
                    let ok = match op {
                        OpCelda::Igual => a == Some(Equal),
                        OpCelda::Distinto => a.is_some_and(|o| o != Equal),
                        OpCelda::Mayor => a == Some(Greater),
                        OpCelda::MayorIgual => matches!(a, Some(Greater | Equal)),
                        OpCelda::Menor => a == Some(Less),
                        OpCelda::MenorIgual => matches!(a, Some(Less | Equal)),
                        OpCelda::Entre | OpCelda::NoEntre => {
                            // Entre en cualquier orden de los dos limites.
                            let dentro = matches!((a, b), (Some(Greater | Equal), Some(Less | Equal)) | (Some(Less | Equal), Some(Greater | Equal)));
                            (a.is_some() && b.is_some()) && (dentro == (*op == OpCelda::Entre))
                        }
                    };
                    (ok, *dxf)
                }
                Regla::Texto { modo, texto: buscado, dxf } => {
                    let t = texto.to_lowercase();
                    let ok = match modo {
                        0 => t.contains(buscado.as_str()),
                        1 => !t.contains(buscado.as_str()),
                        2 => t.starts_with(buscado.as_str()),
                        _ => t.ends_with(buscado.as_str()),
                    };
                    (ok && celda.valor != Valor::Vacio, *dxf)
                }
                Regla::Primeros { rango, porcentaje, ultimos, dxf } => {
                    let ok = num.is_some_and(|v| {
                        let n = e.ordenados.len();
                        let k = if *porcentaje { ((n as f64 * *rango as f64 / 100.0).floor() as usize).max(1) } else { *rango as usize }.min(n);
                        if k == 0 {
                            return false;
                        }
                        if *ultimos { v <= e.ordenados[k - 1] } else { v >= e.ordenados[n - k] }
                    });
                    (ok, *dxf)
                }
                Regla::Promedio { encima, igual, dxf } => {
                    let ok =
                        num.is_some_and(|v| if *encima { v > e.media || (*igual && v == e.media) } else { v < e.media || (*igual && v == e.media) });
                    (ok, *dxf)
                }
                Regla::Escala { puntos } => {
                    if let (Some(v), None) = (num, ef.escala) {
                        let pos: Vec<(f64, Rgb)> = puntos.iter().map(|(p, c)| (self.valor_de(*p, e), *c)).collect();
                        ef.escala = Some(interpolar(v, &pos));
                    }
                    (false, None)
                }
                Regla::Barra { min, max, color, ver_valor } => {
                    if let (Some(v), None) = (num, ef.barra) {
                        let (a, b) = (self.valor_de(*min, e), self.valor_de(*max, e));
                        let t = if b > a { ((v - a) / (b - a)).clamp(0.0, 1.0) } else { 1.0 };
                        // Excel deja un 10 % minimo y un 90 % maximo.
                        ef.barra = Some((0.1 + 0.8 * t as f32, *color));
                        ef.ocultar_valor |= !ver_valor;
                    }
                    (false, None)
                }
            };
            if cumple {
                if let Some(d) = dxf.and_then(|i| dxfs.get(i as usize)) {
                    // Gana la de mas prioridad: solo se rellena lo que falte.
                    let mut nuevo = d.clone();
                    nuevo.sobre(&ef.dxf);
                    ef.dxf = nuevo;
                }
                if *parar {
                    return true;
                }
            }
        }
        false
    }
}

fn interpolar(v: f64, pos: &[(f64, Rgb)]) -> Rgb {
    if v <= pos[0].0 {
        return pos[0].1;
    }
    for w in pos.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            let t = if b > a { ((v - a) / (b - a)) as f32 } else { 1.0 };
            return cb.mezcla(ca, t);
        }
    }
    pos[pos.len() - 1].1
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::xml;

    #[test]
    fn reglas_de_comparar_escalas_y_barras() {
        let x = br#"<x><conditionalFormatting sqref="A1:A10"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>5</formula></cfRule><cfRule type="colorScale" priority="2"><colorScale><cfvo type="min"/><cfvo type="max"/><color rgb="FFFFFFFF"/><color rgb="FF000000"/></colorScale></cfRule><cfRule type="dataBar" priority="3"><dataBar showValue="0"><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule></conditionalFormatting></x>"#;
        let mut cs = leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(cs.len(), 1);
        let c = &mut cs[0];
        c.estadistica = Some(Estadistica { min: 0.0, max: 10.0, media: 5.0, ordenados: (0..=10).map(f64::from).collect() });
        let dxfs = vec![Dxf { negrita: Some(true), ..Default::default() }];
        let mut ef = Efecto::default();
        c.evaluar(&Celda { col: 0, xf: 0, valor: Valor::Numero(10.0) }, "", &dxfs, &mut ef);
        assert_eq!(ef.dxf.negrita, Some(true));
        assert_eq!(ef.escala, Some(Rgb::NEGRO));
        assert!(ef.barra.is_some_and(|(f, _)| (f - 0.9).abs() < 1e-6));
        assert!(ef.ocultar_valor);
        let mut ef = Efecto::default();
        c.evaluar(&Celda { col: 0, xf: 0, valor: Valor::Numero(0.0) }, "", &dxfs, &mut ef);
        assert_eq!(ef.dxf.negrita, None);
        assert_eq!(ef.escala, Some(Rgb::BLANCO));
    }
}
