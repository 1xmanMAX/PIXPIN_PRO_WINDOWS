//! **Correccion de palabras mal leidas** con el diccionario espanol y «Mi
//! vocabulario» (`IndraCorrector.kt`), SOLO cuando la correccion es clara:
//!
//! - la palabra no esta en el diccionario ni en el vocabulario, no lleva
//!   cifras (codigos como U20191A543, importes y fechas no se tocan), tiene
//!   4 letras o mas y su confianza es baja;
//! - el candidato esta a una edicion (diccionario) o a una distancia
//!   pequena ponderada por la confianza de cada letra (vocabulario: cambiar
//!   una letra dudosa cuesta poco, una segura casi una edicion; una tilde
//!   0,1);
//! - es unico, o el modelo de lenguaje de Indra lo prefiere con margen;
//! - el diccionario solo se usa si la pagina es espanola (40 % de sus
//!   palabras estan en el).
//!
//! Banco del escaner (54 imagenes): CER 29,6 → 27,2 con todo el afinado.

use crate::canal::{Linea, Pagina, Palabra};
use crate::lexico::{Hunspell, Vocabulario, clave};
use crate::lm::{Lm, SesgoDePalabras};
use crate::vocab::Vocab;

pub const LARGO_MIN: usize = 4;
/// Ediciones que no son tildes: palabras de 6 letras o mas.
pub const LARGO_MIN_EDICION: usize = 6;
/// Ventaja minima (nats) del candidato segun el LM: minuscula / Mayuscula.
pub const GANANCIA_LM: f64 = 5.0;
pub const GANANCIA_LM_MAYUSCULA: f64 = 10.0;
pub const PALABRAS_MIN: usize = 5;
pub const PARTE_ESPANOLA: f64 = 0.4;
/// Desde esta confianza (media de sus letras) no se corrige.
pub const CONFIANZA_MAX: f32 = 0.9;
pub const MARGEN: f64 = 0.15;
pub const MARGEN_LM: f64 = 1.0;
pub const COSTE_MAX_LEXICO: f64 = 0.75;
/// Bonificacion del LM por palabra completa del diccionario (banco:
/// manuscrito espanol AK137 20,5 → 23,2 % de palabras).
pub const SESGO_LEXICO: f64 = 1.0;
pub const SESGO_VOCABULARIO: f64 = 0.5;
pub const FIN_VOCABULARIO: f64 = 1.0;
const COSTE_INSERTAR: f64 = 0.6;
const COSTE_TILDE: f64 = 0.1;
const LETRAS: &str = "abcdefghijklmnopqrstuvwxyzñáéíóúü";

/// Un cambio hecho, para el registro.
#[derive(Debug, Clone, PartialEq)]
pub struct Cambio {
    pub de: String,
    pub a: String,
    pub origen: &'static str,
}

pub struct Corrector<'a> {
    pub lexico: Option<&'a Hunspell>,
    pub vocabulario: &'a Vocabulario,
}

/// El LM de Indra y su vocabulario, para desempatar.
pub struct Desempate<'a> {
    pub lm: &'a mut Lm,
    pub vocab: &'a Vocab,
}

fn nucleo(t: &str) -> &str {
    t.trim_matches(|c: char| !c.is_alphanumeric())
}

impl Corrector<'_> {
    fn en_lexico(&self, w: &str) -> bool {
        self.lexico.is_some_and(|l| l.contiene(w))
    }

    /// (en el diccionario, palabras de 3 letras o mas) de la pagina.
    pub fn parte_espanola(&self, p: &Pagina) -> (usize, usize) {
        let (mut n, mut si) = (0, 0);
        for l in p.lineas.iter().filter(|l| !l.dudosa) {
            for w in &l.palabras {
                let c = nucleo(&w.texto);
                if c.chars().count() < 3 || !c.chars().all(char::is_alphabetic) {
                    continue;
                }
                n += 1;
                if self.en_lexico(c) {
                    si += 1;
                }
            }
        }
        (si, n)
    }

    /// Corrige la pagina; devuelve los cambios y si era espanola.
    pub fn corregir(&self, p: &mut Pagina, mut lm: Option<Desempate<'_>>) -> (Vec<Cambio>, bool) {
        if self.lexico.is_none() && self.vocabulario.vacio() {
            return (Vec::new(), false);
        }
        let (si, n) = self.parte_espanola(p);
        let espanola =
            self.lexico.is_some() && n >= PALABRAS_MIN && si as f64 >= PARTE_ESPANOLA * n as f64;
        let mut cambios = Vec::new();
        for l in &mut p.lineas {
            self.corregir_linea(l, espanola, &mut lm, &mut cambios);
        }
        (cambios, espanola)
    }

    fn corregir_linea(
        &self,
        l: &mut Linea,
        espanola: bool,
        lm: &mut Option<Desempate<'_>>,
        cambios: &mut Vec<Cambio>,
    ) {
        if l.palabras.is_empty() {
            return;
        }
        let partes: Vec<&str> = l.texto.split(' ').collect();
        if partes.iter().filter(|p| !p.is_empty()).count() != l.palabras.len() {
            return;
        }
        let mut cambio = false;
        for w in &mut l.palabras {
            if let Some((nueva, origen)) = self.corregir_palabra(w, espanola, lm)
                && nueva != w.texto
            {
                cambios.push(Cambio {
                    de: w.texto.clone(),
                    a: nueva.clone(),
                    origen,
                });
                w.texto = nueva;
                w.probs = None;
                cambio = true;
            }
        }
        if cambio {
            let mut k = 0;
            l.texto = partes
                .iter()
                .map(|p| {
                    if p.is_empty() {
                        String::new()
                    } else {
                        k += 1;
                        l.palabras[k - 1].texto.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
    }

    /// (palabra corregida con su puntuacion, origen) o `None`.
    pub fn corregir_palabra(
        &self,
        w: &Palabra,
        espanola: bool,
        lm: &mut Option<Desempate<'_>>,
    ) -> Option<(String, &'static str)> {
        let t: Vec<char> = w.texto.chars().collect();
        let ini = t.iter().position(|c| c.is_alphanumeric())?;
        let fin = t.iter().rposition(|c| c.is_alphanumeric())? + 1;
        let c: String = t[ini..fin].iter().collect();
        let pre: String = t[..ini].iter().collect();
        let post: String = t[fin..].iter().collect();
        let probs: Option<Vec<f32>> = w
            .probs
            .as_ref()
            .filter(|p| p.len() == t.len())
            .map(|p| p[ini..fin].to_vec());
        let largo = c.chars().count();
        if c.chars().any(char::is_numeric) {
            // Solo terminos del vocabulario con cifras, por confusiones
            // letra/cifra.
            if largo < 3 {
                return None;
            }
            let alias = alias_de_cifras(&c);
            for (_, v) in &self.vocabulario.palabras {
                if v.chars().any(char::is_numeric)
                    && v.chars().count() == largo
                    && *v != c
                    && alias_de_cifras(v) == alias
                {
                    return Some((format!("{pre}{v}{post}"), "vocabulario"));
                }
            }
            return None;
        }
        if largo < LARGO_MIN || !c.chars().all(char::is_alphabetic) {
            return None;
        }
        if let Some(v) = self.vocabulario.forma(&c) {
            // Ya es del vocabulario: solo se le ponen sus tildes.
            let f = igualar_mayusculas(&c, v);
            return (f != c && f.to_lowercase() != c.to_lowercase())
                .then(|| (format!("{pre}{f}{post}"), "vocabulario"));
        }
        if w.confianza >= CONFIANZA_MAX {
            return None;
        }
        let en_lex = self.en_lexico(&c);
        // 1) Vocabulario del usuario: distancia ponderada pequena.
        let mut mejor: Option<&str> = None;
        let (mut coste_mejor, mut segundo) = (f64::MAX, f64::MAX);
        for (k, v) in &self.vocabulario.palabras {
            if v.chars().any(char::is_numeric) || v.chars().count() < LARGO_MIN {
                continue;
            }
            if en_lex && *k != clave(&c) {
                continue;
            }
            if distancia(&clave(&c), k) > ediciones_vocabulario(largo) {
                continue;
            }
            let coste = distancia_ponderada(&c, v, probs.as_deref());
            if coste < coste_mejor {
                segundo = coste_mejor;
                coste_mejor = coste;
                mejor = Some(v);
            } else if coste < segundo {
                segundo = coste;
            }
        }
        if let Some(v) = mejor
            && coste_mejor <= coste_max_vocabulario(largo)
            && segundo - coste_mejor >= MARGEN
        {
            return Some((
                format!("{pre}{}{post}", igualar_mayusculas(&c, v)),
                "vocabulario",
            ));
        }
        // 2) Diccionario: una edicion, candidato unico. Tildes: basta con
        //    que sea unico. Otras ediciones: 6 letras y el LM de Indra debe
        //    preferir claramente el candidato (aun mas con Mayuscula).
        if !espanola || en_lex || self.lexico.is_none() {
            return None;
        }
        if largo > 1 && largo <= 4 && c.chars().all(char::is_uppercase) {
            return None; // siglas
        }
        let minus = c.to_lowercase();
        let mut cands: Vec<(String, f64)> = Vec::new();
        for cand in ediciones1(&minus) {
            if self.en_lexico(&cand) {
                let d = distancia_ponderada(&c, &cand, probs.as_deref());
                cands.push((cand, d));
            }
        }
        if cands.is_empty() {
            return None;
        }
        cands.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut top = cands[0].clone();
        if top.1 > COSTE_MAX_LEXICO {
            return None;
        }
        if cands.len() > 1 && cands[1].1 - top.1 < MARGEN {
            let mut empates: Vec<(String, f64)> = cands
                .iter()
                .filter(|x| x.1 - top.1 < MARGEN)
                .map(|x| (x.0.clone(), puntuar(lm, &x.0)))
                .collect();
            empates.sort_by(|a, b| b.1.total_cmp(&a.1));
            if empates[0].1.is_nan() || empates[0].1 - empates[1].1 < MARGEN_LM {
                return None;
            }
            let ganador = empates[0].0.clone();
            top = cands.into_iter().find(|x| x.0 == ganador)?;
        }
        let solo_tilde = clave(&top.0) == clave(&minus);
        if !solo_tilde {
            let g = puntuar(lm, &top.0) - puntuar(lm, &minus);
            if largo < LARGO_MIN_EDICION || g.is_nan() {
                return None;
            }
            let mayus = c.chars().next().is_some_and(char::is_uppercase);
            if g < if mayus {
                GANANCIA_LM_MAYUSCULA
            } else {
                GANANCIA_LM
            } {
                return None;
            }
        }
        Some((
            format!("{pre}{}{post}", igualar_mayusculas(&c, &top.0)),
            if solo_tilde { "tilde" } else { "lexico" },
        ))
    }

    /// El sesgo para la busqueda en haz del LM.
    pub fn sesgo(&self) -> Sesgo<'_> {
        Sesgo {
            c: self,
            lexico: SESGO_LEXICO,
            vocabulario: SESGO_VOCABULARIO,
            fin: FIN_VOCABULARIO,
        }
    }
}

/// log P de la palabra (con espacio delante y detras) segun el LM; NaN sin el.
fn puntuar(lm: &mut Option<Desempate<'_>>, w: &str) -> f64 {
    let Some(d) = lm.as_mut() else {
        return f64::NAN;
    };
    let mut clases = Vec::new();
    if let Some(e) = d.vocab.espacio {
        clases.push(e);
    }
    for ch in w.chars() {
        match d.vocab.indice.get(&ch.to_string()) {
            Some(&c) => clases.push(c),
            None => return f64::NAN,
        }
    }
    if let Some(e) = d.vocab.espacio {
        clases.push(e);
    }
    d.lm.logp_de(&clases)
}

/// Sesgo de palabras: prefijos y palabras del vocabulario, y (suave) las
/// del diccionario.
pub struct Sesgo<'a> {
    c: &'a Corrector<'a>,
    pub lexico: f64,
    pub vocabulario: f64,
    pub fin: f64,
}

impl SesgoDePalabras for Sesgo<'_> {
    fn activo(&self) -> bool {
        !self.c.vocabulario.vacio() || (self.c.lexico.is_some() && self.lexico > 0.0)
    }
    fn paso_max(&self) -> f64 {
        self.vocabulario + self.fin + self.lexico
    }
    fn parcial(&self, w: &str) -> f64 {
        if !self.c.vocabulario.vacio() && self.c.vocabulario.prefijos.contains(&clave(w)) {
            self.vocabulario * w.chars().count() as f64
        } else {
            0.0
        }
    }
    fn completa(&self, w: &str) -> f64 {
        if !self.c.vocabulario.vacio() && self.c.vocabulario.contiene(w) {
            return self.vocabulario * w.chars().count() as f64 + self.fin;
        }
        if self.lexico > 0.0
            && w.chars().count() >= 3
            && w.chars().all(char::is_alphabetic)
            && self.c.en_lexico(w)
        {
            return self.lexico;
        }
        0.0
    }
}

pub fn coste_max_vocabulario(largo: usize) -> f64 {
    (0.3 + 0.12 * largo as f64).min(2.0)
}

/// Ediciones (sin tildes) hacia un termino del vocabulario: 1 hasta 9
/// letras, 2 desde 10, 3 desde 15.
pub fn ediciones_vocabulario(largo: usize) -> usize {
    (largo / 5).max(1)
}

pub fn distancia(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1)
                .min(cur[j - 1] + 1)
                .min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn alias_de_cifras(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .map(|c| match c {
            'O' | 'Q' | 'D' => '0',
            'I' | 'L' | '|' => '1',
            'S' => '5',
            'B' => '8',
            'Z' => '2',
            'G' => '6',
            o => o,
        })
        .collect()
}

/// Copia las mayusculas del original: TODO, Capitalizada o tal cual.
pub fn igualar_mayusculas(orig: &str, cand: &str) -> String {
    let todo =
        orig.chars().count() > 1 && orig.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    if todo {
        return cand.to_uppercase();
    }
    if orig.chars().next().is_some_and(char::is_uppercase) {
        let mut cs = cand.chars();
        return cs
            .next()
            .map(|c| c.to_uppercase().chain(cs).collect())
            .unwrap_or_default();
    }
    cand.to_string()
}

/// Formas a una edicion (borrar, cambiar, insertar, trasponer), minusculas.
pub fn ediciones1(w: &str) -> std::collections::HashSet<String> {
    let c: Vec<char> = w.chars().collect();
    let letras: Vec<char> = LETRAS.chars().collect();
    let s = |v: &[char]| v.iter().collect::<String>();
    let mut o = std::collections::HashSet::new();
    for i in 0..c.len() {
        let mut d = c.clone();
        d.remove(i);
        o.insert(s(&d));
        for &l in &letras {
            if l != c[i] {
                let mut d = c.clone();
                d[i] = l;
                o.insert(s(&d));
            }
        }
        if i + 1 < c.len() {
            let mut d = c.clone();
            d.swap(i, i + 1);
            o.insert(s(&d));
        }
    }
    for i in 0..=c.len() {
        for &l in &letras {
            let mut d = c.clone();
            d.insert(i, l);
            o.insert(s(&d));
        }
    }
    o.remove(w);
    o
}

/// Distancia ponderada de lo leido `o` (probabilidad de cada letra `p`) a
/// `c`: cambiar o borrar cuesta 0,2 + 0,8·p, una tilde 0,1, insertar 0,6.
pub fn distancia_ponderada(o: &str, c: &str, p: Option<&[f32]>) -> f64 {
    let a: Vec<char> = o.to_lowercase().chars().collect();
    let b: Vec<char> = c.to_lowercase().chars().collect();
    let pc = |i: usize| 0.2 + 0.8 * p.and_then(|p| p.get(i)).map_or(0.8, |&v| v as f64);
    let mut prev: Vec<f64> = (0..=b.len()).map(|j| j as f64 * COSTE_INSERTAR).collect();
    for i in 1..=a.len() {
        let mut cur = vec![0.0; b.len() + 1];
        cur[0] = prev[0] + pc(i - 1);
        for j in 1..=b.len() {
            let (x, y) = (a[i - 1], b[j - 1]);
            let sub = if x == y {
                0.0
            } else if clave(&x.to_string()) == clave(&y.to_string()) {
                COSTE_TILDE
            } else {
                pc(i - 1)
            };
            cur[j] = (prev[j - 1] + sub)
                .min(prev[j] + pc(i - 1))
                .min(cur[j - 1] + COSTE_INSERTAR);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn linea(texto: &str, conf: f32, y: f32, dudosa: bool) -> Linea {
        let mut x = 10.0;
        let palabras = texto
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(|w| {
                let n = w.chars().count() as f32;
                let caja = [x, y, x + 12.0 * n, y + 20.0];
                x += 12.0 * n + 10.0;
                Palabra {
                    texto: w.into(),
                    caja,
                    confianza: conf,
                    probs: Some(vec![conf; w.chars().count()]),
                }
            })
            .collect();
        Linea {
            caja: [10.0, y, x, y + 20.0],
            texto: texto.into(),
            confianza: conf,
            palabras,
            dudosa,
        }
    }

    fn pagina(l: Vec<Linea>) -> Pagina {
        Pagina {
            lineas: l,
            bloques: Vec::new(),
            giro: 0,
            inclinacion: 0.0,
            ancho: 1000,
            alto: 1000,
            ms: Default::default(),
        }
    }

    /// Un diccionario de juguete con lo que piden las pruebas del escaner.
    fn lexico() -> Hunspell {
        let dic = "el\nanálisis\nde\nla\nestructura\ncon\nconcreto\narmado\npara\nedificación\nevaluación\ndel\nfuerza\ntotal\n";
        Hunspell::cargar("", dic)
    }

    const CONTEXTO: &str = "el análisis de la estructura con concreto armado para la edificación";

    #[test]
    fn corrige_tildes_en_pagina_espanola() {
        let lx = lexico();
        let v = Vocabulario::default();
        let c = Corrector {
            lexico: Some(&lx),
            vocabulario: &v,
        };
        let mut p = pagina(vec![
            linea(CONTEXTO, 0.99, 0.0, false),
            linea("la evaluacion del analisis", 0.7, 30.0, false),
        ]);
        let (_, esp) = c.corregir(&mut p, None);
        assert!(esp);
        assert_eq!(p.lineas[1].texto, "la evaluación del análisis");
        let mut p = pagina(vec![
            linea(CONTEXTO, 0.99, 0.0, false),
            linea("la evaluacion", 0.97, 30.0, false),
        ]);
        c.corregir(&mut p, None);
        assert_eq!(p.lineas[1].texto, "la evaluacion", "leida con seguridad");
    }

    #[test]
    fn no_toca_codigos_numeros_ni_nombres_ni_paginas_inglesas() {
        let lx = lexico();
        let v = Vocabulario::default();
        let c = Corrector {
            lexico: Some(&lx),
            vocabulario: &v,
        };
        let t = "CÓDIGO U20191A543 total 152.40 Erick Hotev";
        let mut p = pagina(vec![
            linea(CONTEXTO, 0.99, 0.0, false),
            linea(t, 0.5, 30.0, false),
        ]);
        c.corregir(&mut p, None);
        assert_eq!(p.lineas[1].texto, t);
        let mut p = pagina(vec![
            linea(
                "the quick brown fox jumps over the lazy dog and the cat",
                0.99,
                0.0,
                false,
            ),
            linea("the evaluacion form", 0.5, 30.0, false),
        ]);
        let (_, esp) = c.corregir(&mut p, None);
        assert!(!esp);
        assert_eq!(p.lineas[1].texto, "the evaluacion form");
    }

    #[test]
    fn el_vocabulario_del_usuario_manda() {
        let v = Vocabulario::nuevo(&["sismorresistente", "E030", "concreto armado", "sísmica"]);
        let lx = lexico();
        let c = Corrector {
            lexico: Some(&lx),
            vocabulario: &v,
        };
        let mut p = pagina(vec![
            linea(CONTEXTO, 0.99, 0.0, false),
            linea("Ing. Sismorresistenta norma E03O sismica", 0.6, 30.0, false),
        ]);
        let (cambios, _) = c.corregir(&mut p, None);
        assert_eq!(
            p.lineas[1].texto,
            "Ing. Sismorresistente norma E030 sísmica"
        );
        assert!(cambios.iter().all(|x| x.origen == "vocabulario"));
        let mut p = pagina(vec![
            linea(CONTEXTO, 0.99, 0.0, false),
            linea("parado hermano", 0.5, 30.0, false),
        ]);
        c.corregir(&mut p, None);
        assert_eq!(p.lineas[1].texto, "parado hermano", "lejos del termino");
        // Sin diccionario el vocabulario sigue.
        let c = Corrector {
            lexico: None,
            vocabulario: &v,
        };
        let mut p = pagina(vec![linea("sismorresistenta", 0.5, 0.0, false)]);
        c.corregir(&mut p, None);
        assert_eq!(p.lineas[0].texto, "sismorresistente");
    }

    #[test]
    fn el_sesgo_del_lm_premia_el_vocabulario_y_el_diccionario() {
        let v = Vocabulario::nuevo(&["sismorresistente"]);
        let lx = lexico();
        let c = Corrector {
            lexico: Some(&lx),
            vocabulario: &v,
        };
        let s = c.sesgo();
        assert!(s.activo());
        assert!((s.parcial("sism") - 2.0).abs() < 1e-9);
        assert_eq!(s.parcial("sixm"), 0.0);
        assert!((s.completa("sismorresistente") - (0.5 * 16.0 + 1.0)).abs() < 1e-9);
        assert_eq!(s.completa("concreto"), 1.0);
        assert_eq!(s.completa("xyzzy"), 0.0);
        let vacio = Vocabulario::default();
        assert!(
            !Corrector {
                lexico: None,
                vocabulario: &vacio
            }
            .sesgo()
            .activo()
        );
    }

    #[test]
    fn la_distancia_pondera_por_confianza() {
        let dudoso = distancia_ponderada("luerza", "fuerza", Some(&[0.2, 1.0, 1.0, 1.0, 1.0, 1.0]));
        let seguro = distancia_ponderada("luerza", "fuerza", Some(&[1.0; 6]));
        assert!(dudoso < seguro);
        assert!((distancia_ponderada("evaluacion", "evaluación", None) - 0.1).abs() < 1e-9);
        assert!(ediciones1("casa").contains("caso"));
        let lx = lexico();
        let v = Vocabulario::default();
        let c = Corrector {
            lexico: Some(&lx),
            vocabulario: &v,
        };
        let w = Palabra {
            texto: "12,50".into(),
            caja: [0.0; 4],
            confianza: 0.3,
            probs: None,
        };
        assert!(c.corregir_palabra(&w, true, &mut None).is_none());
    }
}
