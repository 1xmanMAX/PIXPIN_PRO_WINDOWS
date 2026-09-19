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

#[cfg(test)]
mod pruebas {
    use super::*;

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
