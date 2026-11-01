//! **Donde se puede partir un renglon sin espacio.**
//!
//! Los que parten texto a mano (el PDF de un chat, el texto pegado en el
//! lienzo) lo hacian solo por los espacios. El chino y el japones no los
//! usan: una frase entera es una «palabra» y no se partia nunca. Aqui se
//! dice donde se puede: entre dos ideogramas o kana, salvo delante de la
//! puntuacion de cierre (`。`, `、`, `」`…) o detras de la de apertura
//! (`「`, `（`…), que es la regla basica del *kinsoku* japones (y la del
//! chino). El coreano escribe con espacios y se queda como estaba.
//!
//! Lo que pinta DirectWrite con `IDWriteTextLayout` ya parte bien solo
//! (sigue UAX #14); esto es para lo que se mide y parte en Rust.

/// Si `c` es de una escritura que se parte entre caracteres: ideogramas Han,
/// hiragana, katakana, la puntuacion CJK y las formas de ancho completo.
pub fn sin_espacios(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FFF   // radicales
        | 0x3000..=0x303F // puntuacion CJK
        | 0x3040..=0x30FF // hiragana y katakana
        | 0x31F0..=0x31FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0xFF00..=0xFFEF // ancho completo
        | 0x20000..=0x3FFFF)
}

/// Lo que no puede empezar un renglon.
fn cierra(c: char) -> bool {
    matches!(
        c,
        '、' | '。'
            | '，'
            | '．'
            | '：'
            | '；'
            | '！'
            | '？'
            | '）'
            | '」'
            | '』'
            | '】'
            | '〉'
            | '》'
            | '〕'
            | '・'
            | 'ー'
            | '々'
            | 'ぁ'
            | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | '…'
            | '!'
            | '?'
            | ')'
            | ','
            | '.'
            | ':'
            | ';'
    )
}

/// Lo que no puede acabar un renglon.
fn abre(c: char) -> bool {
    matches!(c, '（' | '「' | '『' | '【' | '〈' | '《' | '〔' | '(')
}

/// Si entre `a` y `b` (seguidos, sin espacio) se puede partir.
pub fn se_parte_entre(a: char, b: char) -> bool {
    (sin_espacios(a) || sin_espacios(b)) && !cierra(b) && !abre(a)
}

/// Una palabra (lo que hay entre dos espacios) en los trozos por los que
/// se puede partir sin espacio: «hola» es un trozo; «今日は晴れ。» son
/// «今», «日», «は», «晴», «れ。». Una palabra vacia da un trozo vacio, para
/// que quien junta conserve los espacios dobles.
pub fn trozos(palabra: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut desde = 0;
    let mut previo: Option<char> = None;
    for (i, c) in palabra.char_indices() {
        if let Some(a) = previo
            && se_parte_entre(a, c)
        {
            v.push(&palabra[desde..i]);
            desde = i;
        }
        previo = Some(c);
    }
    v.push(&palabra[desde..]);
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_latin_no_se_parte_y_el_japones_entre_caracteres() {
        assert_eq!(trozos("hola"), ["hola"]);
        assert_eq!(trozos(""), [""]);
        assert_eq!(trozos("今日は晴れ。"), ["今", "日", "は", "晴", "れ。"]);
        // Lo que abre va con lo que sigue; lo que cierra, con lo de antes.
        assert_eq!(trozos("「東京」へ"), ["「東", "京」", "へ"]);
        // Mezclado: la palabra latina entera, el chino por caracteres.
        assert_eq!(trozos("PixPin截图"), ["PixPin", "截", "图"]);
        // El coreano va con espacios: una palabra es un trozo.
        assert_eq!(trozos("캡처"), ["캡처"]);
    }
}
