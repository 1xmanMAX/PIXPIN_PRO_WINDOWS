//! **La capa de texto de una hoja**, la que se elige con el raton para
//! copiar o marcar (1-nov-2026): el texto, letra a letra, con la caja de
//! cada letra en fracciones de la hoja (0..1, desde arriba a la izquierda).
//! Es la forma de `pindesk_pdf::texto::PaginaDeTexto`, que es de donde sale
//! en un PDF con texto o en un Word maquetado.
//!
//! Una hoja **escaneada** no tiene texto: lo pone el OCR, que da palabras
//! con su recuadro y no letras. Aqui se pasan esas palabras a letras (la
//! caja de la palabra repartida a partes iguales) para que el lector las
//! trate como cualquier otro texto, y [`a_palabras_enteras`] hace que lo
//! elegido sobre ellas vaya por palabras enteras, como en los pines: el
//! reparto no cuadra con las letras de verdad, y cortar una palabra por la
//! mitad dejaria una marca que no tapa lo que parece.

/// Una palabra reconocida: su caja `[x0, y0, x1, y1]` en pixeles de la
/// imagen que se leyo y su texto.
pub type PalabraLeida = ([f32; 4], String);

/// **Las palabras del OCR como letras** de una hoja de `ancho` x `alto`
/// pixeles: cada renglon, palabra a palabra separadas por un espacio sin
/// caja, y un salto de renglon (sin caja) al final de cada uno.
pub fn de_palabras(
    renglones: &[Vec<PalabraLeida>],
    ancho: f32,
    alto: f32,
) -> (String, Vec<Option<[f32; 4]>>) {
    let (w, h) = (ancho.max(1.0), alto.max(1.0));
    let mut texto = String::new();
    let mut cajas = Vec::new();
    for r in renglones {
        let mut alguna = false;
        for (caja, palabra) in r {
            let n = palabra.chars().count();
            if n == 0 {
                continue;
            }
            if alguna {
                texto.push(' ');
                cajas.push(None);
            }
            alguna = true;
            let paso = (caja[2] - caja[0]) / n as f32;
            for (k, c) in palabra.chars().enumerate() {
                let x0 = caja[0] + paso * k as f32;
                texto.push(c);
                cajas.push(Some([x0 / w, caja[1] / h, (x0 + paso) / w, caja[3] / h]));
            }
        }
        if alguna {
            texto.push('\n');
            cajas.push(None);
        }
    }
    (texto, cajas)
}

/// Si una letra es de palabra (letras, cifras y el guion bajo).
fn de_palabra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// **Lo elegido de `a` a `b`** (letras, `b` sin incluir; en cualquier
/// orden) **llevado a palabras enteras**: el principio retrocede al de su
/// palabra y el final avanza al de la suya. Lo que cae en un blanco se queda
/// donde esta.
pub fn a_palabras_enteras(texto: &[char], a: usize, b: usize) -> (usize, usize) {
    let (mut a, mut b) = (a.min(b).min(texto.len()), a.max(b).min(texto.len()));
    if a == b {
        return (a, b);
    }
    while a > 0 && de_palabra(texto[a]) && de_palabra(texto[a - 1]) {
        a -= 1;
    }
    while b < texto.len() && b > 0 && de_palabra(texto[b - 1]) && de_palabra(texto[b]) {
        b += 1;
    }
    (a, b)
}

/// **Las letras que caen en `zona`** (`[x0, y0, x1, y1]` en las mismas
/// unidades que `cajas`): de la primera a la ultima cuyo centro esta dentro,
/// la ultima sin incluir. Es lo que vuelve a elegir el texto de una marca
/// al hacer clic encima de ella.
pub fn letras_en(cajas: &[Option<[f32; 4]>], zona: [f32; 4]) -> Option<(usize, usize)> {
    let dentro = |c: &[f32; 4]| {
        let (cx, cy) = ((c[0] + c[2]) / 2.0, (c[1] + c[3]) / 2.0);
        cx >= zona[0] && cx <= zona[2] && cy >= zona[1] && cy <= zona[3]
    };
    let primera = cajas.iter().position(|c| c.as_ref().is_some_and(dentro))?;
    let ultima = cajas.iter().rposition(|c| c.as_ref().is_some_and(dentro))?;
    Some((primera, ultima + 1))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn p(x0: f32, x1: f32, y: f32, t: &str) -> PalabraLeida {
        ([x0, y, x1, y + 20.0], t.to_string())
    }

    #[test]
    fn las_palabras_pasan_a_letras_con_su_caja_repartida() {
        let (texto, cajas) = de_palabras(
            &[
                vec![
                    p(100.0, 140.0, 10.0, "hola"),
                    p(160.0, 200.0, 10.0, "mundo"),
                ],
                vec![p(100.0, 120.0, 50.0, "si")],
            ],
            1000.0,
            500.0,
        );
        assert_eq!(texto, "hola mundo\nsi\n");
        assert_eq!(cajas.len(), texto.chars().count());
        // La «h» ocupa la primera cuarta parte de «hola».
        let h = cajas[0].unwrap();
        assert!((h[0] - 0.1).abs() < 1e-5 && (h[2] - 0.11).abs() < 1e-5);
        assert!((h[1] - 0.02).abs() < 1e-5 && (h[3] - 0.06).abs() < 1e-5);
        // El espacio y el salto no tienen caja.
        assert_eq!(cajas[4], None);
        assert_eq!(cajas[10], None);
        // Caso negativo: un renglon sin palabras no deja ni el salto.
        let (t, c) = de_palabras(&[vec![], vec![p(0.0, 10.0, 0.0, "")]], 10.0, 10.0);
        assert!(t.is_empty() && c.is_empty());
    }

    #[test]
    fn arrastrar_por_media_palabra_elige_la_palabra_entera() {
        let t: Vec<char> = "uno dos tres\ncuatro".chars().collect();
        // De la «n» de uno a la «o» de dos (sin incluir): «uno dos».
        assert_eq!(a_palabras_enteras(&t, 1, 6), (0, 7));
        // Al reves, lo mismo.
        assert_eq!(a_palabras_enteras(&t, 6, 1), (0, 7));
        // Hasta media palabra del renglon siguiente.
        assert_eq!(a_palabras_enteras(&t, 9, 15), (8, 19));
        // Empezando en un blanco: se queda en el blanco.
        assert_eq!(a_palabras_enteras(&t, 3, 5), (3, 7));
        // Caso negativo: nada elegido sigue sin nada, y fuera del texto no revienta.
        assert_eq!(a_palabras_enteras(&t, 2, 2), (2, 2));
        assert_eq!(a_palabras_enteras(&t, 50, 99), (19, 19));
    }

    #[test]
    fn las_letras_de_una_zona_van_de_la_primera_a_la_ultima() {
        let (_, cajas) = de_palabras(
            &[
                vec![
                    p(100.0, 140.0, 10.0, "hola"),
                    p(160.0, 200.0, 10.0, "mundo"),
                ],
                vec![p(100.0, 120.0, 50.0, "si")],
            ],
            1000.0,
            500.0,
        );
        // «mundo» entero, con un poco de aire alrededor.
        assert_eq!(letras_en(&cajas, [0.155, 0.01, 0.205, 0.07]), Some((5, 10)));
        // Las dos lineas.
        assert_eq!(letras_en(&cajas, [0.0, 0.0, 1.0, 1.0]), Some((0, 13)));
        // Caso negativo: una zona en blanco no elige nada.
        assert_eq!(letras_en(&cajas, [0.5, 0.5, 0.6, 0.6]), None);
    }
}
