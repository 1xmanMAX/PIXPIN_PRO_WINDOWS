//! Lo que el universo sabe de un archivo del chat (D237): lo que se pinta y
//! lo que se busca, nada mas. Un `Mensaje` entero pesa diez veces esto.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ClaseLuna {
    Imagen,
    #[default]
    Archivo,
    Voz,
    Dibujo,
    Pagina,
    MiniApp,
    Proyecto,
    Nota,
}

impl ClaseLuna {
    /// La palabra tal como la escribe el movil (`cuaderno::Clase`). Lo que
    /// no se conoce entra como archivo generico (D200).
    pub fn de_palabra(p: &str) -> ClaseLuna {
        match p {
            "IMAGEN" => ClaseLuna::Imagen,
            "VOZ" => ClaseLuna::Voz,
            "DIBUJO" => ClaseLuna::Dibujo,
            "PAGINA" => ClaseLuna::Pagina,
            "MINIAPP" => ClaseLuna::MiniApp,
            "PROYECTO" => ClaseLuna::Proyecto,
            "NOTA" => ClaseLuna::Nota,
            _ => ClaseLuna::Archivo,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FichaLuna {
    /// `Mensaje::codigo_unico`.
    pub codigo: String,
    pub proyecto: String,
    pub clase: ClaseLuna,
    pub nombre: String,
    /// Relativa a la carpeta del proyecto.
    pub ruta: Option<String>,
    pub bytes: i64,
    pub cuando: i64,
    /// `47·K7Q2`.
    pub codigo_chat: Option<String>,
    pub extracto: String,
    /// Nombre + extracto, ya normalizado (`buscar::normalizar`).
    pub busqueda: String,
    /// Si el fichero esta en este equipo. Falso con ruta absoluta del movil
    /// o fichero borrado: la luna se pinta como fantasma (D219).
    pub en_equipo: bool,
    /// La hoja de un `Dibujo` o una `Pagina` (`Mensaje::referencia`).
    pub referencia: Option<String>,
    /// Una nota que es solo emoticonos (`es_solo_emoji`): se pinta como el
    /// emoji suelto, sin caja de nota, como el movil (v0.85).
    pub solo_emoji: bool,
}

pub const EXTRACTO: usize = 200;

pub fn extracto(texto: &str) -> String {
    texto.chars().take(EXTRACTO).collect()
}

/// D200.
pub fn es_colocable(clase: ClaseLuna, en_buzon: bool, incluir_notas: bool) -> bool {
    if en_buzon {
        return false;
    }
    clase != ClaseLuna::Nota || incluir_notas
}

/// **Si un texto es solo emoticonos** (`esSoloEmoji` del movil,
/// `Universo.kt:851`): un «👍» no es una nota con texto, es un gesto, y en el
/// universo se ve como el emoji suelto que es.
///
/// Mismas reglas: hasta 16 unidades UTF-16 sin los espacios de los lados,
/// con las uniones (ZWJ), los selectores de variante, la tecla de numero y
/// los tonos de piel permitidos, y al menos un simbolo. El movil pregunta la
/// categoria Unicode «otro simbolo», que Rust no trae: aqui van los bloques
/// donde viven los emoji (desde U+1F000, flechas a simbolos varios, y los
/// sueltos de siempre: ©, ®, ‼, ⁉, ™, ℹ, 〰, 〽, ㊗, ㊙).
pub fn es_solo_emoji(texto: &str) -> bool {
    let limpio = texto.trim();
    if limpio.is_empty() || limpio.encode_utf16().count() > 16 {
        return false;
    }
    let mut hay_emoji = false;
    for c in limpio.chars() {
        let p = c as u32;
        match p {
            0x200D | 0xFE0F | 0xFE0E | 0x20E3 => {}
            _ if c.is_whitespace() => {}
            0x1F000.. => hay_emoji = true,
            0x2190..=0x2BFF
            | 0x00A9
            | 0x00AE
            | 0x203C
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299 => hay_emoji = true,
            _ => return false,
        }
    }
    hay_emoji
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_pulgar_un_corazon_y_una_familia_con_uniones_son_solo_emoji() {
        assert!(es_solo_emoji("👍"));
        assert!(es_solo_emoji(" ❤️ "));
        assert!(es_solo_emoji("👨‍👩‍👧"));
        assert!(es_solo_emoji("👍🏽 🔥"));
        assert!(es_solo_emoji("✅"));
    }

    #[test]
    fn un_texto_con_letras_o_numeros_o_vacio_o_largo_no_es_solo_emoji() {
        assert!(!es_solo_emoji("ok 👍"));
        assert!(!es_solo_emoji("3"));
        assert!(!es_solo_emoji(""));
        assert!(!es_solo_emoji("   "));
        // Nueve emoji de dos unidades UTF-16 pasan de 16.
        assert!(!es_solo_emoji(&"😀".repeat(9)));
        assert!(es_solo_emoji(&"😀".repeat(8)));
    }

    #[test]
    fn las_palabras_del_movil_se_traducen_y_lo_desconocido_es_archivo() {
        assert_eq!(ClaseLuna::de_palabra("IMAGEN"), ClaseLuna::Imagen);
        assert_eq!(ClaseLuna::de_palabra("PAGINA"), ClaseLuna::Pagina);
        assert_eq!(ClaseLuna::de_palabra("MINIAPP"), ClaseLuna::MiniApp);
        assert_eq!(ClaseLuna::de_palabra("HOLOGRAMA"), ClaseLuna::Archivo);
    }

    #[test]
    fn las_notas_solo_entran_si_se_piden_y_el_buzon_nunca() {
        assert!(es_colocable(ClaseLuna::Imagen, false, false));
        assert!(!es_colocable(ClaseLuna::Nota, false, false));
        assert!(es_colocable(ClaseLuna::Nota, false, true));
        assert!(!es_colocable(ClaseLuna::Imagen, true, true));
    }

    #[test]
    fn el_extracto_corta_en_doscientas_letras_sin_partir_un_caracter() {
        let largo: String = "ñ".repeat(300);
        let e = extracto(&largo);
        assert_eq!(e.chars().count(), EXTRACTO);
        assert_eq!(extracto("corto"), "corto");
    }
}
