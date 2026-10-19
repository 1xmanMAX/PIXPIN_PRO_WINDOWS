//! **El supervisor del calcar** (18-oct-2026). Pedido del usuario, ante una
//! lamina de Polinomios con texto encima de texto y recuadros en blanco:
//! «mejora la logica de los agentes e implementa un supervisor de los
//! agentes». Hasta aqui la comprobacion era de pixeles (`comparar`): no sabe
//! si una letra sobra ni si una formula se leyo mal.
//!
//! Al final, con todo ya en el lienzo, se pinta el resultado y, de las zonas
//! que menos se parecen al papel, se le ensena al modelo cada una lado a lado
//! (el original a la izquierda, lo nuestro a la derecha). Contesta si esta
//! bien y, si no, que pasa y el texto bueno renglon a renglon (con sus
//! formulas en `$...$` y su negrita), o que se ponga el recorte. La zona se
//! rehace con eso. Una sola vuelta, todas a la vez.

use pixpin_ia::hoja::{Caja, Tipo, imagen, montaje, vision};
use pixpin_ia::motor::Motor;
use serde_json::Value;

/// Cuantas zonas mira como mucho (las que peor se parecen).
pub const MAXIMO: usize = 10;
/// Desde cuanto parecido no hace falta mirarla.
pub const BASTA: f64 = 0.93;

const SISTEMA: &str = "Eres el supervisor de una herramienta que pasa una hoja (una lamina, un apunte) a un lienzo EDITABLE. \
Recibes UNA imagen partida en dos por una raya roja: a la IZQUIERDA el trozo ORIGINAL de la hoja; a la DERECHA como lo dejo la herramienta. \
Se EXIGENTE: compara palabra a palabra y simbolo a simbolo. Revisa, en orden: \
1) ¿falta algo? (un renglon, una formula, un recuadro vacio donde habia una formula); \
2) ¿sobra o esta mal escrito? (letras sueltas, cifras sin sentido como «c -900-500=400» en vez de C50°=90°−50°=40°, codigo LaTeX a la vista como \\underbrace, texto encima de texto); \
3) ¿las formulas dicen lo mismo? (exponentes, subindices, fracciones, raices, grados °, letras griegas); \
4) ¿la negrita esta donde esta? \
Si hay cualquier diferencia de contenido, NO esta bien. Solo dan igual: otra letra, un tamano un poco distinto y la marca de agua \
(letras grandes y grises de fondo, que NO se copia). \
Responde SOLO un JSON: {\"bien\":true} si esta bien. Si no: {\"bien\":false,\"problema\":\"en una frase\",\"renglones\":[\"...\"],\"recorte\":false}. \
En \"renglones\" va el texto CORRECTO del original, un elemento por renglon visual, en espanol con sus tildes, las formulas en linea entre $...$ con LaTeX \
(x^{4}, P_{(x)}, \\frac{x+2}{y-1}, \\sqrt{y}, \\sin z) y la negrita entre **...**. Pon \"recorte\":true (y renglones vacio) solo si es un dibujo o un \
esquema que no se puede escribir como texto.";

/// Lo que dijo el supervisor de una zona.
#[derive(Debug, Clone, PartialEq)]
pub enum Veredicto {
    Bien,
    /// El texto bueno, renglon a renglon.
    Renglones(Vec<String>, String),
    /// Mejor el recorte de la hoja.
    Recorte(String),
}

/// Lee su respuesta (`None`: no se entiende).
pub fn leer(t: &str) -> Option<Veredicto> {
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    let v: Value = serde_json::from_str(t.get(a..=b)?).ok()?;
    if v["bien"].as_bool() == Some(true) {
        return Some(Veredicto::Bien);
    }
    let problema = v["problema"].as_str().unwrap_or("").to_string();
    if v["recorte"].as_bool() == Some(true) {
        return Some(Veredicto::Recorte(problema));
    }
    let r: Vec<String> = v["renglones"].as_array()?.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
    (!r.is_empty()).then_some(Veredicto::Renglones(r, problema))
}

/// **El original y lo nuestro lado a lado**, con una raya roja en medio, a
/// lo mas 1400 px de ancho, en PNG.
pub fn lado_a_lado(original: (&[u8], u32, u32), nuestro: (&[u8], u32, u32)) -> Option<Vec<u8>> {
    let (w1, h1) = (original.1, original.2);
    let (w2, h2) = (nuestro.1, nuestro.2);
    if w1 == 0 || h1 == 0 || w2 == 0 || h2 == 0 {
        return None;
    }
    let raya = 6u32;
    let (w, h) = (w1 + raya + w2, h1.max(h2));
    let mut px = vec![255u8; (w * h * 4) as usize];
    let poner = |px: &mut Vec<u8>, src: &[u8], sw: u32, sh: u32, x0: u32| {
        for y in 0..sh {
            let d = ((y * w + x0) * 4) as usize;
            let o = (y * sw * 4) as usize;
            px[d..d + (sw * 4) as usize].copy_from_slice(&src[o..o + (sw * 4) as usize]);
        }
    };
    poner(&mut px, original.0, w1, h1, 0);
    poner(&mut px, nuestro.0, w2, h2, w1 + raya);
    for y in 0..h {
        for x in w1..w1 + raya {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[220, 30, 30, 255]);
        }
    }
    let (px, w, h) = montaje::reducir(&px, w, h, 1400);
    pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px }).ok()
}

/// Pregunta por una zona (con su recorte original y el nuestro).
pub fn mirar(motor: &dyn Motor, original: (&[u8], u32, u32), nuestro: (&[u8], u32, u32), que_es: &str) -> Option<Veredicto> {
    let png = lado_a_lado(original, nuestro)?;
    let pregunta = format!("Es {que_es}. ¿Lo de la derecha dice lo mismo que lo de la izquierda?");
    let t = vision::preguntar_con(motor, SISTEMA, png, &pregunta, 1500).ok()?;
    leer(&t)
}

/// Las zonas que se miran: las que peor se parecen (de las que se pueden
/// rehacer: texto, formulas, tablas, dibujos), hasta [`MAXIMO`].
pub fn candidatas(parecidos: &[(usize, f64)], tipo: impl Fn(usize) -> Option<Tipo>) -> Vec<usize> {
    let mut v: Vec<(usize, f64)> = parecidos
        .iter()
        .copied()
        .filter(|(_, p)| *p < BASTA)
        .filter(|(z, _)| matches!(tipo(*z), Some(Tipo::Parrafo | Tipo::Encabezado | Tipo::Formula | Tipo::Tabla | Tipo::Dibujo | Tipo::Diagrama | Tipo::Dudosa)))
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v.into_iter().take(MAXIMO).map(|(z, _)| z).collect()
}

/// El recorte de una caja con aire (para el original y lo pintado).
pub fn recorte(rgba: &[u8], w: u32, h: u32, c: &Caja, aire: f64) -> (Vec<u8>, u32, u32) {
    imagen::recortar(rgba, w, h, &c.crecer(aire))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn entiende_lo_que_dice_el_supervisor() {
        assert_eq!(leer("```json\n{\"bien\": true}\n```"), Some(Veredicto::Bien));
        assert_eq!(
            leer(r#"{"bien":false,"problema":"falta la formula","renglones":["$P_{(x)}=x^{4}+6$ es un polinomio", ""],"recorte":false}"#),
            Some(Veredicto::Renglones(vec!["$P_{(x)}=x^{4}+6$ es un polinomio".into()], "falta la formula".into()))
        );
        assert_eq!(leer(r#"{"bien":false,"problema":"es un esquema","renglones":[],"recorte":true}"#), Some(Veredicto::Recorte("es un esquema".into())));
        assert_eq!(leer("no se"), None);
        assert_eq!(leer(r#"{"bien":false,"renglones":[]}"#), None, "sin nada que hacer, nada");
    }

    #[test]
    fn las_candidatas_son_las_peores_de_lo_que_se_puede_rehacer() {
        let p = [(0, 0.99), (1, 0.4), (2, 0.7), (3, 0.2)];
        let tipo = |z: usize| Some(if z == 3 { Tipo::Foto } else { Tipo::Parrafo });
        assert_eq!(candidatas(&p, tipo), vec![1, 2]);
    }

    #[test]
    fn el_lado_a_lado_lleva_la_raya_roja() {
        let a = vec![0u8; 10 * 5 * 4];
        let b = vec![255u8; 8 * 7 * 4];
        let png = lado_a_lado((&a, 10, 5), (&b, 8, 7)).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
