//! Vocabulario del reconocedor CTC (`indra_ocr/vocab.py`, solo lo que usa
//! la inferencia; `IndraVocab.kt`). Clase 0 = blanco; clase `i + 1` =
//! `fichas[i]` (un caracter o un comando LaTeX como `\alpha`).

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Vocab {
    pub fichas: Vec<String>,
    /// ficha → clase (`i + 1`).
    pub indice: HashMap<String, u32>,
    con_comandos: bool,
    /// La clase del espacio (para cortar palabras y celdas).
    pub espacio: Option<u32>,
}

impl Vocab {
    pub fn nuevo(fichas: Vec<String>) -> Vocab {
        let indice: HashMap<String, u32> = fichas
            .iter()
            .enumerate()
            .map(|(i, c)| (c.clone(), i as u32 + 1))
            .collect();
        let con_comandos = fichas.iter().any(|c| c.chars().count() > 1);
        let espacio = indice.get(" ").copied();
        Vocab {
            fichas,
            indice,
            con_comandos,
            espacio,
        }
    }

    /// Desde la cadena JSON `{"blank": 0, "chars": [...]}` de `meta.json`.
    pub fn de_json(s: &str) -> Result<Vocab, String> {
        let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let blanco = v.get("blank").and_then(|b| b.as_i64()).unwrap_or(0);
        if blanco != 0 {
            return Err("solo vocabularios con blank = 0".into());
        }
        let fichas = v
            .get("chars")
            .and_then(|c| c.as_array())
            .ok_or("el vocabulario no trae «chars»")?
            .iter()
            .map(|c| c.as_str().unwrap_or("").to_string())
            .collect();
        Ok(Vocab::nuevo(fichas))
    }

    /// La ficha de una clase (sin el blanco).
    pub fn ficha(&self, clase: u32) -> Option<&str> {
        (clase >= 1)
            .then(|| self.fichas.get(clase as usize - 1))
            .flatten()
            .map(|s| s.as_str())
    }

    pub fn decodificar(&self, clases: &[u32]) -> String {
        let fichas: Vec<&str> = clases.iter().filter_map(|&c| self.ficha(c)).collect();
        if !self.con_comandos {
            return fichas.concat();
        }
        let mut s = String::new();
        for (k, t) in fichas.iter().enumerate() {
            s.push_str(t);
            // «\alpha» + «x» no debe leerse «\alphax».
            let sig = fichas.get(k + 1).copied().unwrap_or("");
            if t.len() > 2
                && t.starts_with('\\')
                && t[1..].chars().all(|c| c.is_alphabetic())
                && sig.chars().next().is_some_and(|c| c.is_alphabetic())
            {
                s.push(' ');
            }
        }
        s
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn decodifica_y_separa_los_comandos() {
        // IndraUnitTest.vocabDecodificaYEspacioDeComandos
        let v =
            Vocab::de_json(r#"{"blank": 0, "chars": [" ", "a", "b", "\\alpha", "x"]}"#).unwrap();
        assert_eq!(v.espacio, Some(1));
        assert_eq!(v.decodificar(&[2, 3, 1, 2]), "ab a");
        assert_eq!(v.decodificar(&[4, 5]), "\\alpha x");
        assert_eq!(v.decodificar(&[0, 99]), "");
        assert!(Vocab::de_json(r#"{"blank": 3, "chars": []}"#).is_err());
    }
}
