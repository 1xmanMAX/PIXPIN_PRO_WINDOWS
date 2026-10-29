//! **Guiones de casos sueltos** (29-oct-2026, «Más casos»): lo que no es una
//! reunion frase a frase (una captura, una nota, una rafaga de fotos, un
//! aviso) se escribe como una lista de casos en la cabecera del guion:
//!
//! ```text
//! # chat: Obra Calle Mayor | reforma del piso: planos y presupuestos
//! # caso: c01
//! # ventana: presupuesto.pdf - Visor
//! # texto: Partida 3: alicatado del baño...
//! # espera: c01 → obra calle mayor
//! ```
//!
//! `# caso: id` empieza un caso; las claves que siguen son suyas hasta el
//! siguiente. Las de antes del primero son de todos (`Guion::valor`). En
//! los valores, `\n` es un salto de linea. Las esperas son las de siempre:
//! cada prototipo apunta un hecho «id → lo decidido» y la espera lo busca.

use std::path::Path;

use pixpin_typesafe::Imagen;

use super::Guion;

/// Un caso: su id y sus claves, en orden.
#[derive(Debug, Clone)]
pub struct Caso {
    pub id: String,
    pub campos: Vec<(String, String)>,
}

impl Caso {
    pub fn valor(&self, k: &str) -> Option<&str> {
        self.campos
            .iter()
            .find(|(c, _)| c == k)
            .map(|(_, v)| v.as_str())
    }

    /// El valor o vacio.
    pub fn texto(&self, k: &str) -> String {
        self.valor(k).unwrap_or("").to_string()
    }

    pub fn valores(&self, k: &str) -> Vec<String> {
        self.campos
            .iter()
            .filter(|(c, _)| c == k)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// `# numero: 3` (o `None`).
    pub fn numero(&self, k: &str) -> Option<f64> {
        self.valor(k)?.trim().parse().ok()
    }
}

/// **Los casos del guion**, en orden.
pub fn casos(g: &Guion) -> Vec<Caso> {
    let mut v: Vec<Caso> = Vec::new();
    for (k, valor) in &g.cabecera {
        let valor = valor.replace("\\n", "\n");
        if k == "caso" {
            v.push(Caso {
                id: valor.trim().to_string(),
                campos: Vec::new(),
            });
        } else if let Some(c) = v.last_mut() {
            c.campos.push((k.clone(), valor));
        }
    }
    v
}

/// Una imagen del guion: la ruta, relativa a la carpeta del guion.
pub fn imagen(g: &Guion, ruta: &str) -> Imagen {
    let base = g.ruta.parent().unwrap_or(Path::new("."));
    let p = base.join(ruta.trim());
    let bytes = std::fs::read(&p).unwrap_or_else(|e| {
        eprintln!("{}: {e}", p.display());
        std::process::exit(2)
    });
    let formato = match p.extension().and_then(|e| e.to_str()) {
        Some("png") => "png",
        _ => "jpg",
    };
    Imagen {
        formato: formato.to_string(),
        bytes,
    }
}

/// «0.92» con dos decimales.
pub fn p(x: f64) -> String {
    format!("{x:.2}")
}
