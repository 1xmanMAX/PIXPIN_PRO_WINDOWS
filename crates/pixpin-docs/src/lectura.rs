//! **Leer a gusto**: el tamano de la letra y los marcadores con emoticono.
//! Puerto de `motor/Lectura.kt` del movil (v0.65-v0.66).
//!
//! Un marcador guarda **en que fraccion del documento** esta lo que habia
//! arriba de la pantalla al ponerlo, no un numero de pixeles: asi sigue
//! cayendo (casi) en el mismo parrafo aunque luego se agrande la letra y
//! todo el documento se alargue.
//!
//! Lo que el movil guarda en sus ajustes, aqui se guarda **junto al
//! documento**, en un fichero hermano `<nombre>.pixpin-lectura`. Es lo que
//! pidio el usuario, y ademas tiene una ventaja: los marcadores viajan con
//! el documento si se copia la carpeta, y no se quedan en un ajuste que
//! nadie sabe donde esta. Una linea por cosa, sin JSON que arrastrar, igual
//! que el movil.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    /// Cuando se puso, en milisegundos. Sirve de identidad.
    pub id: u64,
    /// De 0 a 1.
    pub fraccion: f32,
    pub emoji: String,
}

pub const TAMANO_MIN: u32 = 70;
pub const TAMANO_MAX: u32 = 250;

/// Los tamanos de la barra de puntos, en tanto por ciento: cada punto, uno.
pub const TAMANOS: [u32; 9] = [80, 90, 100, 115, 130, 150, 175, 200, 230];

/// Los emoticonos con los que se marca, en el orden del movil.
pub const EMOJIS: [&str; 12] = [
    "🔖", "⭐", "❤️", "❗", "❓", "💡", "📌", "✅", "🔥", "👀", "✏️", "🏁",
];

/// Mas de esto no se guardan: la tira lateral deja de distinguirse.
pub const MARCADORES: usize = 24;

pub fn tamano_valido(t: u32) -> u32 {
    t.clamp(TAMANO_MIN, TAMANO_MAX)
}

/// El siguiente tamano de la barra de puntos, hacia arriba o hacia abajo.
pub fn tamano_vecino(actual: u32, hacia_arriba: bool) -> u32 {
    let i = punto_del_tamano(actual);
    let siguiente = if hacia_arriba {
        (i + 1).min(TAMANOS.len() - 1)
    } else {
        i.saturating_sub(1)
    };
    TAMANOS[siguiente]
}

/// Que punto de la barra corresponde al tamano de ahora.
pub fn punto_del_tamano(actual: u32) -> usize {
    TAMANOS
        .iter()
        .position(|t| *t >= actual)
        .unwrap_or(TAMANOS.len() - 1)
}

/// Todo lo que se recuerda de un documento entre una lectura y la
/// siguiente.
#[derive(Debug, Clone, PartialEq)]
pub struct Ajustes {
    pub tamano: u32,
    /// Por donde se iba, de 0 a 1.
    pub sitio: f32,
    pub marcadores: Vec<Marcador>,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            tamano: 100,
            sitio: 0.0,
            marcadores: Vec::new(),
        }
    }
}

/// Uno mas, **en el orden del documento**, que es el de los puntos del
/// lateral.
pub fn con_marcador(lista: &[Marcador], fraccion: f32, emoji: &str, ahora: u64) -> Vec<Marcador> {
    let mut salida = lista.to_vec();
    salida.push(Marcador {
        id: ahora,
        fraccion: fraccion.clamp(0.0, 1.0),
        emoji: if emoji.trim().is_empty() {
            EMOJIS[0].to_string()
        } else {
            emoji.to_string()
        },
    });
    ordenar(&mut salida);
    // Los mas viejos del documento se van si sobran, como en el movil
    // (`takeLast`).
    if salida.len() > MARCADORES {
        salida.drain(..salida.len() - MARCADORES);
    }
    salida
}

fn ordenar(lista: &mut [Marcador]) {
    lista.sort_by(|a, b| {
        a.fraccion
            .partial_cmp(&b.fraccion)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// El fichero hermano donde vive todo esto.
pub fn ruta_de_ajustes(documento: &Path) -> PathBuf {
    let mut nombre = documento
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    nombre.push_str(".pixpin-lectura");
    documento.with_file_name(nombre)
}

/// Lee los ajustes de un documento. Si no hay fichero, o esta roto, salen
/// los de fabrica: no leer es tan normal como leer.
pub fn leer(documento: &Path) -> Ajustes {
    match std::fs::read_to_string(ruta_de_ajustes(documento)) {
        Ok(t) => de_texto(&t),
        Err(_) => Ajustes::default(),
    }
}

/// Guarda los ajustes junto al documento. Si no se puede escribir (un
/// documento en un sitio de solo lectura), se dice y no pasa nada mas: la
/// lectura no depende de esto.
pub fn escribir(documento: &Path, a: &Ajustes) -> std::io::Result<()> {
    std::fs::write(ruta_de_ajustes(documento), a_texto(a))
}

pub fn a_texto(a: &Ajustes) -> String {
    let marcadores = a
        .marcadores
        .iter()
        .map(|m| format!("{}:{}:{}", m.id, m.fraccion, m.emoji))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "pixpin-lectura 1\ntamano {}\nsitio {}\nmarcadores {marcadores}\n",
        a.tamano, a.sitio
    )
}

pub fn de_texto(texto: &str) -> Ajustes {
    let mut a = Ajustes::default();
    for linea in texto.lines() {
        let (clave, valor) = linea.split_once(' ').unwrap_or((linea, ""));
        match clave {
            "tamano" => a.tamano = tamano_valido(valor.trim().parse().unwrap_or(100)),
            "sitio" => a.sitio = valor.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0),
            "marcadores" => a.marcadores = marcadores_de_texto(valor),
            _ => {}
        }
    }
    a
}

/// Guardados en una linea: `id:fraccion:emoji` separados por `|`, igual que
/// en el movil.
pub fn marcadores_de_texto(texto: &str) -> Vec<Marcador> {
    let mut salida: Vec<Marcador> = texto
        .split('|')
        .filter_map(|trozo| {
            // Solo dos cortes: un emoticono puede traer dos puntos dentro
            // (los hay con variantes).
            let (id, resto) = trozo.split_once(':')?;
            let (fraccion, emoji) = resto.split_once(':').unwrap_or((resto, ""));
            Some(Marcador {
                id: id.trim().parse().ok()?,
                fraccion: fraccion.trim().parse::<f32>().ok()?.clamp(0.0, 1.0),
                emoji: if emoji.trim().is_empty() {
                    EMOJIS[0].to_string()
                } else {
                    emoji.to_string()
                },
            })
        })
        .collect();
    ordenar(&mut salida);
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(id: u64, f: f32) -> Marcador {
        Marcador {
            id,
            fraccion: f,
            emoji: "🔖".into(),
        }
    }

    #[test]
    fn un_marcador_nuevo_entra_por_donde_le_toca_en_el_documento() {
        let lista = vec![m(1, 0.1), m(2, 0.8)];
        let con = con_marcador(&lista, 0.5, "⭐", 3);
        let f: Vec<f32> = con.iter().map(|x| x.fraccion).collect();
        assert_eq!(f, vec![0.1, 0.5, 0.8]);
        assert_eq!(con[1].emoji, "⭐");
    }

    #[test]
    fn una_fraccion_imposible_se_mete_en_el_documento() {
        let con = con_marcador(&[], 5.0, "⭐", 1);
        assert_eq!(con[0].fraccion, 1.0);
        let con = con_marcador(&[], -3.0, "⭐", 1);
        assert_eq!(con[0].fraccion, 0.0);
    }

    #[test]
    fn nunca_se_guardan_mas_de_veinticuatro() {
        let mut lista: Vec<Marcador> = (0..MARCADORES as u64)
            .map(|i| m(i, i as f32 / 100.0))
            .collect();
        lista = con_marcador(&lista, 0.99, "🏁", 999);
        assert_eq!(lista.len(), MARCADORES);
        assert_eq!(
            lista.last().map(|x| x.id),
            Some(999),
            "el nuevo tiene que estar; el que sobra es el primero"
        );
    }

    #[test]
    fn se_guardan_y_se_leen_igual() {
        let a = Ajustes {
            tamano: 130,
            sitio: 0.25,
            marcadores: vec![m(1, 0.1), m(2, 0.9)],
        };
        let vuelta = de_texto(&a_texto(&a));
        assert_eq!(vuelta, a);
    }

    #[test]
    fn un_fichero_roto_no_rompe_la_lectura() {
        let a = de_texto("basura\ntamano no-es-un-numero\nsitio lejos\nmarcadores ;;;");
        assert_eq!(a, Ajustes::default());
        assert_eq!(de_texto(""), Ajustes::default());
    }

    #[test]
    fn un_marcador_a_medio_escribir_se_tira_y_los_demas_no() {
        let lista = marcadores_de_texto("1:0.2:🔖|basura|3:0.5:⭐|:0.7:x");
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].id, 1);
        assert_eq!(lista[1].id, 3);
    }

    #[test]
    fn un_marcador_sin_emoticono_se_queda_con_el_de_siempre() {
        let lista = marcadores_de_texto("1:0.2:");
        assert_eq!(lista[0].emoji, EMOJIS[0]);
    }

    #[test]
    fn el_tamano_se_queda_entre_el_minimo_y_el_maximo() {
        assert_eq!(tamano_valido(10), TAMANO_MIN);
        assert_eq!(tamano_valido(999), TAMANO_MAX);
        assert_eq!(tamano_valido(120), 120);
    }

    #[test]
    fn la_barra_de_puntos_no_se_sale_por_los_extremos() {
        assert_eq!(tamano_vecino(TAMANOS[0], false), TAMANOS[0]);
        assert_eq!(
            tamano_vecino(TAMANOS[TAMANOS.len() - 1], true),
            TAMANOS[TAMANOS.len() - 1]
        );
        assert_eq!(tamano_vecino(100, true), 115);
        assert_eq!(tamano_vecino(100, false), 90);
    }

    #[test]
    fn el_fichero_de_ajustes_es_hermano_del_documento() {
        let r = ruta_de_ajustes(Path::new("C:/apuntes/tema 1.docx"));
        assert_eq!(
            r.file_name().unwrap().to_string_lossy(),
            "tema 1.docx.pixpin-lectura"
        );
        assert_eq!(r.parent(), Path::new("C:/apuntes/tema 1.docx").parent());
    }
}
