//! **El asistente dentro de una conversacion**: lo que no depende de la
//! ventana (que se le manda al modelo, que foto y que adjuntos ve, y como se
//! escribe la burbuja en vivo). La ventana lo engancha en
//! `ventana_chat/asistente.rs`.
//!
//! - En el chat fijo «Asistente» todo lo que se escribe es para el.
//! - En cualquier otro chat, `@asistente` al principio o en medio le
//!   pregunta sobre lo que hay ahi (ve los ultimos mensajes del chat).
//! - Su respuesta es una nota normal del cuaderno, con `aparato = "IA"`
//!   (la chapa dice `#48·IA`): se guarda, se busca y viaja como cualquier
//!   otra, sin un formato nuevo que el movil no sepa leer.

use std::path::PathBuf;

use pixpin_ia::agente::{Evento, Final};
use pixpin_ia::herramientas::{AYUDA, CALCAR, DIBUJAR, GRAFICA, OCR};
use pixpin_proyecto::cuaderno::{Clase, Mensaje as Nota};

use super::turno::Adjunto;

/// El «aparato» de las notas del asistente.
pub const APARATO: &str = "IA";

/// La mencion que lo llama desde otro chat.
pub const MENCION: &str = "@asistente";

pub fn es_del_asistente(m: &Nota) -> bool {
    m.aparato.as_deref() == Some(APARATO)
}

fn es_nota(m: &Nota) -> bool {
    matches!(m.clase, None | Some(Clase::Nota))
}

/// Si el texto lo menciona, la pregunta sin la mencion. Sin distinguir
/// mayusculas; la mencion tiene que ser palabra entera (`@asistentes` no).
pub fn mencion(texto: &str) -> Option<String> {
    let bajo = texto.to_lowercase();
    let mut desde = 0;
    while let Some(i) = bajo[desde..].find(MENCION) {
        let i = desde + i;
        let fin = i + MENCION.len();
        let antes_ok = i == 0 || !bajo[..i].chars().next_back().is_some_and(char::is_alphanumeric);
        let despues_ok = !bajo[fin..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
        if antes_ok && despues_ok {
            let resto = format!("{}{}", &texto[..i], &texto[fin..]);
            let limpio = resto
                .trim()
                .trim_start_matches([',', ':'])
                .trim()
                .to_string();
            return Some(limpio);
        }
        desde = fin;
    }
    None
}

/// La conversacion de antes, para el modelo: `(es_del_usuario, texto)` de
/// las notas (las fotos y ficheros van aparte).
pub fn conversacion(anteriores: &[Nota]) -> Vec<(bool, String)> {
    anteriores
        .iter()
        .filter(|m| es_nota(m) && !m.texto.trim().is_empty())
        .map(|m| (!es_del_asistente(m), m.texto.clone()))
        .collect()
}

/// La foto que se le enseña: la ultima imagen escrita desde la ultima
/// respuesta del asistente (adjuntar una foto y luego preguntar).
pub fn foto_reciente(mensajes: &[Nota]) -> Option<&Nota> {
    for m in mensajes.iter().rev() {
        if es_del_asistente(m) {
            return None;
        }
        if m.clase == Some(Clase::Imagen) {
            return Some(m);
        }
    }
    None
}

/// Los adjuntos que el OCR puede leer (fotos y PDF), del mas viejo al mas
/// nuevo. `ruta` resuelve donde esta el fichero de cada mensaje.
pub fn adjuntos(mensajes: &[Nota], ruta: impl Fn(&Nota) -> Option<PathBuf>) -> Vec<Adjunto> {
    mensajes
        .iter()
        .filter(|m| matches!(m.clase, Some(Clase::Imagen) | Some(Clase::Archivo)))
        .filter_map(|m| {
            let r = ruta(m)?;
            (super::ocr::es_pdf(&r) || super::ocr::es_imagen(&r)).then(|| Adjunto {
                nombre: if m.nombre.is_empty() {
                    r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                } else {
                    m.nombre.clone()
                },
                ruta: r,
            })
        })
        .collect()
}

/// El ultimo lienzo de la conversacion, si hay alguno.
pub fn ultimo_lienzo(mensajes: &[Nota]) -> Option<usize> {
    mensajes
        .iter()
        .enumerate()
        .rev()
        .find(|(_, m)| m.clase == Some(Clase::Dibujo) && m.referencia.as_deref().is_some_and(|r| !r.is_empty()))
        .map(|(i, _)| i)
}

/// **La burbuja que se escribe en vivo.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnVivo {
    pub texto: String,
    /// «Dibujando…» mientras el modelo usa una herramienta.
    pub estado: Option<String>,
    /// Lo que fue mal por el camino (se dice al final, en una linea).
    pub fallos: Vec<String>,
    pub acabado: bool,
}

/// Lo que se ensena mientras se usa cada herramienta.
pub fn rotulo_de(nombre: &str) -> String {
    match nombre {
        DIBUJAR => "✏️ dibujando en el lienzo…",
        GRAFICA => "📊 haciendo la gráfica…",
        OCR => "🔎 leyendo el documento…",
        CALCAR => "🖼️ calcando la imagen…",
        AYUDA => "📖 mirando la ayuda…",
        _ => "⚙️ trabajando…",
    }
    .to_string()
}

impl EnVivo {
    pub fn aplicar(&mut self, e: &Evento) {
        match e {
            Evento::Texto(t) => {
                self.texto.push_str(t);
                self.estado = None;
            }
            Evento::Llamando(n) => self.estado = Some(rotulo_de(n)),
            Evento::Herramienta { llamada, .. } => {
                // Lo que diga despues va en su linea, no pegado a lo de antes.
                if !self.texto.is_empty() && !self.texto.ends_with('\n') {
                    self.texto.push('\n');
                }
                self.estado = Some(rotulo_de(llamada.nombre()));
            }
            Evento::Resultado { ok, texto, .. } => {
                if !ok {
                    self.fallos.push(texto.trim_start_matches("ERROR: ").to_string());
                }
                self.estado = Some("💭 …".into());
            }
            Evento::Reintento { .. } => self.estado = Some("🔁 corrigiendo…".into()),
        }
    }

    /// El final del turno.
    pub fn acabar(&mut self, r: &Result<Final, String>) {
        self.acabado = true;
        self.estado = None;
        match r {
            Ok(f) => {
                if self.texto.trim().is_empty() {
                    self.texto = f.texto.clone();
                }
                if f.cortado {
                    if !self.texto.is_empty() {
                        self.texto.push(' ');
                    }
                    self.texto.push_str("⏹ (parado)");
                }
            }
            Err(e) => {
                if !self.texto.is_empty() {
                    self.texto.push_str("\n\n");
                }
                self.texto.push_str(&format!("⚠️ No pude contestar: {e}"));
            }
        }
    }

    /// El texto de la burbuja tal como se ve ahora.
    pub fn mostrar(&self) -> String {
        let mut s = self.texto.trim_end().to_string();
        if let Some(e) = &self.estado {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(e);
        } else if !self.acabado {
            s.push_str(" ▍");
        }
        if self.acabado && !self.fallos.is_empty() {
            s.push_str(&format!("\n⚠️ {}", self.fallos.join(" · ")));
        }
        s.trim_start().to_string()
    }
}
