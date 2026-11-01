//! **Dictar al chat** (29-oct-2026, prototipo `dictado_a_chat`): «anota
//! esto en el chat de la obra» → en que chat (Choice sobre los nombres de
//! los chats, con un Noul de «nombra alguno») y que se anota. El texto **no
//! se genera**: el codigo saca los candidatos ([`textos`]: lo que sigue en
//! la frase despues de «:» o de «que», y la frase anterior para «esto») y
//! Jev elige cual.
//!
//! **Tabla** ([`decidir`]):
//!
//! | `dictado` | `dictado_chat_dicho` | se hace |
//! |---|---|---|
//! | ≥ SI | ≥ SI | nota en el chat elegido |
//! | ≥ SI | < SI | nota sin chat (se pregunta al usuario cual) |
//! | ≥ SI, sin nada que anotar | — | nada (no hay texto) |
//! | entre NO y SI | — | nada, se apunta como dudoso |
//! | ≤ NO | — | nada |

use crate::preguntas::vivo as q;
use crate::preguntas::{NO, SI};
use crate::vivo::{Dictado, Frase};

/// Lo que se podria anotar: (clave de la opcion, texto).
pub fn textos(frase: &Frase, antes: &[Frase]) -> Vec<(String, String)> {
    let mut v = Vec::new();
    let d = frase.dice.trim();
    let bajo = d.to_lowercase();
    let resto = if let Some(i) = d.find(':') {
        Some(&d[i + 1..])
    } else {
        // «… que faltan tornillos»: lo de despues del primer «que» que
        // sigue a «chat» o «grupo» (o del primero, si no se nombran).
        let desde = ["chat", "grupo"]
            .iter()
            .filter_map(|w| bajo.find(w))
            .min()
            .unwrap_or(0);
        bajo.get(desde..)
            .and_then(|b| b.find(" que "))
            .and_then(|i| d.get(desde + i + 5..))
    };
    if let Some(r) = resto
        .map(|r| r.trim().trim_end_matches(['.', '!']).trim())
        .filter(|r| r.split_whitespace().count() >= 2)
    {
        v.push((q::LO_QUE_SIGUE.to_string(), r.to_string()));
    }
    if let Some(a) = antes.last().filter(|a| !a.dice.trim().is_empty()) {
        v.push((q::LA_ANTERIOR.to_string(), crate::recortar(&a.dice, 300)));
    }
    v
}

/// Una nota para un chat.
#[derive(Debug, Clone, PartialEq)]
pub struct Nota {
    pub ms: i64,
    /// `None`: no se dijo el chat (se pregunta).
    pub chat: Option<String>,
    pub texto: String,
    pub p: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    Nota(Nota),
    SinTexto,
    Dudoso(f64),
    Nada,
}

/// **La tabla.**
pub fn decidir(frase: &Frase, d: Option<&Dictado>, chats: &[String]) -> Resultado {
    let Some(d) = d else {
        return Resultado::Nada;
    };
    if d.p <= NO {
        return Resultado::Nada;
    }
    if d.p < SI {
        return Resultado::Dudoso(d.p);
    }
    let Some(texto) = d.texto.clone() else {
        return Resultado::SinTexto;
    };
    Resultado::Nota(Nota {
        ms: frase.ms,
        chat: (d.chat_dicho >= SI)
            .then(|| chats.get(d.chat).cloned())
            .flatten(),
        texto,
        p: d.p,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(dice: &str) -> Frase {
        Frase {
            ms: 0,
            quien: "Ana".into(),
            dice: dice.into(),
        }
    }

    #[test]
    fn los_textos_que_se_podrian_anotar() {
        let antes = [f("el cemento llega el jueves a las 8")];
        let t = textos(&f("anota esto en el chat de la obra"), &antes);
        assert_eq!(
            t,
            vec![(
                q::LA_ANTERIOR.into(),
                "el cemento llega el jueves a las 8".into()
            )]
        );
        let t = textos(&f("pon en el grupo de compras que faltan tornillos."), &[]);
        assert_eq!(t, vec![(q::LO_QUE_SIGUE.into(), "faltan tornillos".into())]);
        let t = textos(
            &f("Anota en el chat de la obra: Llega la grúa el lunes"),
            &antes,
        );
        assert_eq!(t[0].1, "Llega la grúa el lunes");
        assert_eq!(t.len(), 2);
        assert!(textos(&f("anótalo"), &[]).is_empty());
    }

    #[test]
    fn la_tabla_del_dictado() {
        let chats = vec!["Obra Calle Lima".to_string(), "Compras".to_string()];
        let d = |p, dicho| Dictado {
            p,
            chat_dicho: dicho,
            chat: 0,
            texto: Some("faltan tornillos".into()),
        };
        let fr = f("x");
        let Resultado::Nota(n) = decidir(&fr, Some(&d(0.9, 0.9)), &chats) else {
            panic!()
        };
        assert_eq!(n.chat.as_deref(), Some("Obra Calle Lima"));
        let Resultado::Nota(n) = decidir(&fr, Some(&d(0.9, 0.2)), &chats) else {
            panic!()
        };
        assert_eq!(n.chat, None);
        assert_eq!(
            decidir(&fr, Some(&d(0.5, 0.9)), &chats),
            Resultado::Dudoso(0.5)
        );
        assert_eq!(decidir(&fr, Some(&d(0.1, 0.9)), &chats), Resultado::Nada);
        let mut sin = d(0.9, 0.9);
        sin.texto = None;
        assert_eq!(decidir(&fr, Some(&sin), &chats), Resultado::SinTexto);
    }
}
