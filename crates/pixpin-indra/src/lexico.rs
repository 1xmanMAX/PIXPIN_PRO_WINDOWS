//! **El diccionario espanol y «Mi vocabulario»** de la correccion
//! (`Hunspell.kt` y `UserVocabulary.kt` del escaner).
//!
//! El diccionario es el hunspell `es_ES` de RLA-ES tal como lo trae
//! LibreOffice (triple licencia GPL/LGPL/MPL; se usa bajo la MPL). Solo se
//! pregunta si una palabra existe: raices + reglas de afijos, sin expandir
//! el millon de formas (~2 MB de memoria en vez de ~80). Admite lo que usa
//! `es_ES`: `FLAG UTF-8`, `PFX`/`SFX` con condicion, producto cruzado y un
//! nivel de sufijo de continuacion («-cion/S» → «-ciones»).

use std::collections::{HashMap, HashSet};

/// Condicion de una regla («[^aeiou]r», «.», «ecer»), al final (sufijo) o
/// al principio (prefijo).
#[derive(Debug, Clone)]
struct Condicion {
    /// `None` = cualquiera; `Some((negada, conjunto))`.
    partes: Vec<Option<(bool, Vec<char>)>>,
}

impl Condicion {
    fn nueva(spec: &str) -> Condicion {
        let c: Vec<char> = spec.chars().collect();
        let mut partes = Vec::new();
        let mut i = 0;
        while i < c.len() {
            if c[i] == '[' {
                let j = c[i..]
                    .iter()
                    .position(|&x| x == ']')
                    .map_or(c.len(), |p| i + p);
                let neg = c.get(i + 1) == Some(&'^');
                let desde = if neg { i + 2 } else { i + 1 };
                partes.push(Some((neg, c[desde.min(j)..j].to_vec())));
                i = j + 1;
            } else {
                partes.push(if c[i] == '.' {
                    None
                } else {
                    Some((false, vec![c[i]]))
                });
                i += 1;
            }
        }
        Condicion { partes }
    }

    fn vale(p: &Option<(bool, Vec<char>)>, c: char) -> bool {
        match p {
            None => true,
            Some((neg, set)) => set.contains(&c) != *neg,
        }
    }

    fn al_final(&self, w: &[char]) -> bool {
        if self.partes.iter().all(Option::is_none) {
            return true;
        }
        if w.len() < self.partes.len() {
            return false;
        }
        let o = w.len() - self.partes.len();
        self.partes
            .iter()
            .enumerate()
            .all(|(k, p)| Self::vale(p, w[o + k]))
    }

    fn al_principio(&self, w: &[char]) -> bool {
        if self.partes.iter().all(Option::is_none) {
            return true;
        }
        if w.len() < self.partes.len() {
            return false;
        }
        self.partes
            .iter()
            .enumerate()
            .all(|(k, p)| Self::vale(p, w[k]))
    }
}

#[derive(Debug, Clone)]
struct Afijo {
    bandera: char,
    quita: String,
    cond: Condicion,
    cruzado: bool,
    continua: Vec<char>,
}

/// Diccionario hunspell minimo: solo «¿existe esta palabra?».
pub struct Hunspell {
    raices: HashMap<String, Vec<char>>,
    sufijos: HashMap<String, Vec<Afijo>>,
    prefijos: HashMap<String, Vec<Afijo>>,
    max_suf: usize,
    max_pre: usize,
    continuaciones: HashSet<char>,
}

impl Hunspell {
    /// Desde el texto de `.aff` y `.dic` (UTF-8).
    pub fn cargar(aff: &str, dic: &str) -> Hunspell {
        let mut sufijos: HashMap<String, Vec<Afijo>> = HashMap::new();
        let mut prefijos: HashMap<String, Vec<Afijo>> = HashMap::new();
        let mut cruzado: HashMap<String, bool> = HashMap::new();
        let (mut max_suf, mut max_pre) = (0, 0);
        for linea in aff.lines() {
            let f: Vec<&str> = linea.split_whitespace().collect();
            if f.len() < 4 || (f[0] != "SFX" && f[0] != "PFX") {
                continue;
            }
            let clave = format!("{}{}", f[0], f[1]);
            if f.len() == 4 && (f[2] == "Y" || f[2] == "N") {
                cruzado.insert(clave, f[2] == "Y");
                continue;
            }
            if f.len() < 5 {
                continue;
            }
            let quita = if f[2] == "0" { "" } else { f[2] };
            let (anade, cont) = f[3].split_once('/').unwrap_or((f[3], ""));
            let anade = if anade == "0" { "" } else { anade };
            let a = Afijo {
                bandera: f[1].chars().next().unwrap_or(' '),
                quita: quita.to_string(),
                cond: Condicion::nueva(f[4]),
                cruzado: cruzado.get(&clave).copied().unwrap_or(false),
                continua: cont.chars().collect(),
            };
            let n = anade.chars().count();
            if f[0] == "SFX" {
                sufijos.entry(anade.to_string()).or_default().push(a);
                max_suf = max_suf.max(n);
            } else {
                prefijos.entry(anade.to_string()).or_default().push(a);
                max_pre = max_pre.max(n);
            }
        }
        let mut raices: HashMap<String, Vec<char>> = HashMap::with_capacity(80_000);
        for (i, linea) in dic.lines().enumerate() {
            let l = linea.trim();
            if i == 0 && !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let entrada = l.split([' ', '\t']).next().unwrap_or(l);
            let (w, fl) = entrada.split_once('/').unwrap_or((entrada, ""));
            raices.entry(w.to_string()).or_default().extend(fl.chars());
        }
        let continuaciones = sufijos
            .values()
            .flatten()
            .flat_map(|a| a.continua.iter().copied())
            .collect();
        Hunspell {
            raices,
            sufijos,
            prefijos,
            max_suf,
            max_pre,
            continuaciones,
        }
    }

    pub fn tamano(&self) -> usize {
        self.raices.len()
    }

    /// ¿Esta la palabra (tal cual, en minuscula, o capitalizada si viene en
    /// MAYUSCULAS: «MADRID» → «Madrid»)?
    pub fn contiene(&self, w: &str) -> bool {
        if w.is_empty() {
            return false;
        }
        if self.mira(w) {
            return true;
        }
        let minus = w.to_lowercase();
        if minus != w && self.mira(&minus) {
            return true;
        }
        if w.chars().count() > 1 && w.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) {
            let mut cs = minus.chars();
            let cap: String = cs
                .next()
                .map(|c| c.to_uppercase().chain(cs).collect())
                .unwrap_or_default();
            if self.mira(&cap) {
                return true;
            }
        }
        false
    }

    fn tiene(&self, raiz: &str, bandera: char) -> bool {
        self.raices.get(raiz).is_some_and(|f| f.contains(&bandera))
    }

    fn mira(&self, w: &str) -> bool {
        if self.raices.contains_key(w) {
            return true;
        }
        let c: Vec<char> = w.chars().collect();
        if self.con_sufijo(&c, None, false) {
            return true;
        }
        for l in 0..=self.max_pre.min(c.len()) {
            let pre: String = c[..l].iter().collect();
            let Some(reglas) = self.prefijos.get(&pre) else {
                continue;
            };
            let resto: String = c[l..].iter().collect();
            for r in reglas {
                let raiz = format!("{}{resto}", r.quita);
                let rc: Vec<char> = raiz.chars().collect();
                if raiz.is_empty() || !r.cond.al_principio(&rc) {
                    continue;
                }
                if self.tiene(&raiz, r.bandera) {
                    return true;
                }
                if r.cruzado && self.con_sufijo(&rc, Some(r.bandera), false) {
                    return true;
                }
            }
        }
        false
    }

    /// ¿`w` = raiz + sufijo (o + sufijo de continuacion)? `necesita`: la
    /// raiz debe llevar tambien esa bandera (prefijo cruzado).
    fn con_sufijo(&self, w: &[char], necesita: Option<char>, segundo: bool) -> bool {
        for l in 0..=self.max_suf.min(w.len()) {
            let suf: String = w[w.len() - l..].iter().collect();
            let Some(reglas) = self.sufijos.get(&suf) else {
                continue;
            };
            let base: String = w[..w.len() - l].iter().collect();
            for r in reglas {
                if necesita.is_some() && !r.cruzado {
                    continue;
                }
                let raiz = format!("{base}{}", r.quita);
                let rc: Vec<char> = raiz.chars().collect();
                if raiz.is_empty() || !r.cond.al_final(&rc) {
                    continue;
                }
                let ok = |s: &Hunspell| {
                    s.tiene(&raiz, r.bandera) && necesita.is_none_or(|n| s.tiene(&raiz, n))
                };
                if ok(self) {
                    return true;
                }
                if !segundo
                    && self.continuaciones.contains(&r.bandera)
                    && self.con_continuacion(&rc, r.bandera, necesita)
                {
                    return true;
                }
            }
        }
        false
    }

    fn con_continuacion(&self, w: &[char], bandera2: char, necesita: Option<char>) -> bool {
        for l in 0..=self.max_suf.min(w.len()) {
            let suf: String = w[w.len() - l..].iter().collect();
            let Some(reglas) = self.sufijos.get(&suf) else {
                continue;
            };
            let base: String = w[..w.len() - l].iter().collect();
            for r in reglas {
                if !r.continua.contains(&bandera2) || (necesita.is_some() && !r.cruzado) {
                    continue;
                }
                let raiz = format!("{base}{}", r.quita);
                let rc: Vec<char> = raiz.chars().collect();
                if raiz.is_empty() || !r.cond.al_final(&rc) {
                    continue;
                }
                if self.tiene(&raiz, r.bandera) && necesita.is_none_or(|n| self.tiene(&raiz, n)) {
                    return true;
                }
            }
        }
        false
    }
}

/// Minusculas sin tildes ni dieresis (la ene se queda).
pub fn clave(w: &str) -> String {
    w.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ý' | 'ÿ' => 'y',
            otro => otro,
        })
        .collect()
}

/// «Mi vocabulario»: terminos del usuario (nombres, cursos, siglas como
/// «E030») que mandan en la correccion y en la busqueda en haz.
#[derive(Debug, Clone, Default)]
pub struct Vocabulario {
    pub terminos: Vec<String>,
    /// Las palabras de los terminos (como las escribio), por su [`clave`],
    /// en el orden en que salen.
    pub palabras: Vec<(String, String)>,
    pub prefijos: HashSet<String>,
}

/// Palabras mas cortas no cuentan (demasiado faciles de confundir).
pub const LARGO_MIN_VOCABULARIO: usize = 3;

impl Vocabulario {
    pub fn nuevo<S: AsRef<str>>(terminos: &[S]) -> Vocabulario {
        let mut t: Vec<String> = Vec::new();
        for x in terminos {
            let x = x.as_ref().split_whitespace().collect::<Vec<_>>().join(" ");
            if !x.is_empty() && !t.contains(&x) {
                t.push(x);
            }
        }
        let mut palabras: Vec<(String, String)> = Vec::new();
        for termino in &t {
            for w in termino.split(' ') {
                let nucleo = w.trim_matches(|c: char| !c.is_alphanumeric());
                if nucleo.chars().count() >= LARGO_MIN_VOCABULARIO {
                    let k = clave(nucleo);
                    if !palabras.iter().any(|(kk, _)| *kk == k) {
                        palabras.push((k, nucleo.to_string()));
                    }
                }
            }
        }
        let mut prefijos = HashSet::new();
        for (k, _) in &palabras {
            let c: Vec<char> = k.chars().collect();
            for i in 1..=c.len() {
                prefijos.insert(c[..i].iter().collect());
            }
        }
        Vocabulario {
            terminos: t,
            palabras,
            prefijos,
        }
    }

    /// Del texto del ajuste: un termino por linea o separados por comas.
    pub fn de_texto(texto: &str) -> Vocabulario {
        let v: Vec<&str> = texto
            .split(['\n', ',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        Vocabulario::nuevo(&v)
    }

    pub fn vacio(&self) -> bool {
        self.palabras.is_empty()
    }

    /// La forma del usuario de una palabra, por su clave.
    pub fn forma(&self, w: &str) -> Option<&str> {
        let k = clave(w);
        self.palabras
            .iter()
            .find(|(kk, _)| *kk == k)
            .map(|(_, f)| f.as_str())
    }

    pub fn contiene(&self, w: &str) -> bool {
        self.forma(w).is_some()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn hunspell_minimo_con_afijos() {
        // IndraAfinadoTest.hunspellMinimoConAfijos
        let aff = "SET UTF-8\nFLAG UTF-8\nPFX r Y 1\nPFX r 0 re .\nSFX S Y 2\nSFX S 0 s [aeiou]\nSFX S 0 es [^aeiou]\nSFX A Y 1\nSFX A r ción/S ar\n";
        let dic = "4\ncasa/S\nhacer/r\nevaluar/A\nMadrid\n";
        let h = Hunspell::cargar(aff, dic);
        assert!(h.contiene("casa"));
        assert!(h.contiene("casas"));
        assert!(!h.contiene("casaes"));
        assert!(h.contiene("rehacer"));
        assert!(h.contiene("evaluación"));
        assert!(h.contiene("evaluaciónes"), "continuacion /S");
        assert!(h.contiene("Madrid"));
        assert!(h.contiene("MADRID"));
        assert!(h.contiene("Casas"));
        assert!(!h.contiene("xadrid"));
        assert_eq!(h.tamano(), 4);
    }

    #[test]
    fn el_vocabulario_quita_duplicados_y_compara_sin_tildes() {
        let v = Vocabulario::nuevo(&[
            "sismorresistente",
            "E030",
            "concreto armado",
            "sísmica",
            " ",
            "sísmica",
        ]);
        assert_eq!(
            v.terminos,
            ["sismorresistente", "E030", "concreto armado", "sísmica"]
        );
        assert!(v.contiene("Armado"));
        assert!(v.contiene("sismica"), "sin tildes");
        assert_eq!(v.forma("SISMICA"), Some("sísmica"));
        assert!(v.prefijos.contains("sism"));
        assert_eq!(clave("Ñandú"), "ñandu");
        let v = Vocabulario::de_texto("uno, dos\ntres;  ");
        assert_eq!(v.terminos, ["uno", "dos", "tres"]);
    }
}
