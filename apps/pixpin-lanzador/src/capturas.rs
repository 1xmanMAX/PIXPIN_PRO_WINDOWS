//! **Las capturas de pantalla, en Flow**: `p capturas [texto]`, `p galeria`,
//! `p ultima`.
//!
//! Se leen de `<raiz>/capturas` (las mismas que la galeria de la app,
//! `galeria_capturas::listar`: la mas nueva primero, por fecha) y su
//! caducidad de `<raiz>/capturas-caducidad.json` (`caducidad_capturas`):
//! `{"desde": <ms>, "conservadas": ["captura-0001.png", ...]}`. Una captura
//! se va a los siete dias de lo MAS TARDE entre su fecha y `desde`; las
//! conservadas no se van.
//!
//! Solo se lee: conservar, copiar o borrar es un pedido a la app.

use crate::normalizar::{normalizar, puntuar};
use crate::resultados::{glifo, pedido, pedido_ventana, Accion, Contexto, Resultado, MAXIMO};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Las extensiones que lista la galeria (`galeria_capturas::EXTENSIONES`).
pub const EXTENSIONES: [&str; 7] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4"];
/// Lo que aguanta una captura antes de irse sola (`caducidad_capturas::DIAS`).
pub const DIAS: i64 = 7;
const DIA_MS: i64 = 86_400_000;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Registro {
    desde: i64,
    conservadas: Vec<String>,
}

/// Una captura de la carpeta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captura {
    pub ruta: PathBuf,
    pub nombre: String,
    /// Su fecha (la del fichero), en ms UTC.
    pub cuando: i64,
    /// Cuando se va sola; `None`: conservada.
    pub se_va: Option<i64>,
}

impl Captura {
    pub fn es_video(&self) -> bool {
        self.ruta.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
    }
}

pub fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("capturas")
}

/// Cuando se va una captura de fecha `cuando`, con el registro `desde`.
pub fn se_va_el(cuando: i64, desde: i64, conservada: bool) -> Option<i64> {
    (!conservada).then_some(cuando.max(desde) + DIAS * DIA_MS)
}

fn ms(t: std::time::SystemTime) -> i64 {
    t.duration_since(std::time::SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Las capturas, la mas nueva primero. Sin registro de caducidad (la app aun
/// no lo escribio), `desde` es ahora: es lo que la app apuntara al mirarlo.
pub fn leer(raiz: &Path, ahora: i64) -> Vec<Captura> {
    let Ok(dir) = std::fs::read_dir(carpeta(raiz)) else {
        return Vec::new();
    };
    let registro = std::fs::read_to_string(raiz.join("capturas-caducidad.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Registro>(t.trim_start_matches('\u{feff}')).ok());
    let (desde, conservadas) = match registro {
        Some(r) if r.desde > 0 => (r.desde, r.conservadas),
        Some(r) => (ahora, r.conservadas),
        None => (ahora, Vec::new()),
    };
    let mut v: Vec<Captura> = dir
        .flatten()
        .filter_map(|e| {
            let ruta = e.path();
            let meta = e.metadata().ok()?;
            let ext = ruta.extension().and_then(|x| x.to_str())?;
            // Una ruta reservada y aun vacia es una captura escribiendose.
            if !meta.is_file() || meta.len() == 0 || !EXTENSIONES.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                return None;
            }
            let nombre = ruta.file_name()?.to_string_lossy().to_string();
            let cuando = meta.modified().map(ms).unwrap_or(0);
            let se_va = se_va_el(cuando, desde, conservadas.contains(&nombre));
            Some(Captura { ruta, nombre, cuando, se_va })
        })
        .collect();
    // Como la galeria: por fecha y, a igual hora, el nombre al reves.
    v.sort_by(|a, b| b.cuando.cmp(&a.cuando).then_with(|| b.ruta.cmp(&a.ruta)));
    v
}

/// «Captura · hace 2 h · se borra el 10 oct» / «… · Conservada».
pub fn subtitulo(c: &Captura, ahora: i64) -> String {
    let tipo = if c.es_video() { "Vídeo" } else { "Captura" };
    let cuando = crate::fecha::hace_con_horas(c.cuando, ahora);
    let fin = match c.se_va {
        None => "Conservada".to_string(),
        Some(t) if t <= ahora => "se borra ya".to_string(),
        Some(t) => match crate::fecha::dia_y_mes(t) {
            d if d.is_empty() => String::new(),
            d => format!("se borra el {d}"),
        },
    };
    [tipo, cuando.as_str(), fin.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" · ")
}

/// Una captura como resultado: Intro la saca como pin (un video se abre).
pub fn resultado(c: &Captura, ctx: &Contexto) -> Resultado {
    let ruta = c.ruta.to_string_lossy().to_string();
    let (glifo, accion) = if c.es_video() {
        (glifo::VIDEO, Accion::Pedido(pedido("abrir", json!({ "que": { "tipo": "fichero", "ruta": ruta } }))))
    } else {
        (glifo::IMAGEN, Accion::Pedido(pedido("pinear", json!({ "ruta": ruta }))))
    };
    let mut r = Resultado::nuevo(&c.nombre, subtitulo(c, ctx.ahora), glifo, accion);
    if !c.es_video() {
        r.icono = Some(ruta.clone());
    }
    r.con_fichero(&c.ruta);
    r.copiar = Some(ruta.clone());
    r.clave = Some(format!("captura/{}", c.nombre));
    r.autocompletar = Some(ctx.consulta(&format!("capturas {}", c.nombre)));
    r.con_menu(json!({ "tipo": "captura", "ruta": ruta, "conservada": c.se_va.is_none() }));
    r
}

/// «Abrir la galería de capturas».
pub fn resultado_galeria() -> Resultado {
    let mut r = Resultado::nuevo(
        "Abrir la galería de capturas",
        "Todas, en la ventana de la galería",
        glifo::GALERIA,
        Accion::Pedido(pedido_ventana("galeria")),
    );
    r.clave = Some("ventana/galeria".into());
    r
}

/// Las capturas que encajan con `filtro` (todas si esta vacio), de la mas
/// nueva a la mas vieja; si no hay, una fila que lo dice.
pub fn lista(ctx: &Contexto, filtro: &str) -> Vec<Resultado> {
    let todas = ctx.raiz_de_datos().map(|r| leer(&r, ctx.ahora)).unwrap_or_default();
    let q = normalizar(filtro.trim());
    let v: Vec<Resultado> = todas
        .iter()
        .filter(|c| q.is_empty() || puntuar(&q, &normalizar(&c.nombre), "") > 0)
        .take(MAXIMO - 1)
        .map(|c| resultado(c, ctx))
        .collect();
    if !v.is_empty() {
        return v;
    }
    if todas.is_empty() {
        let mut r = Resultado::nuevo(
            "No hay capturas todavía",
            "Intro: capturar una zona de la pantalla",
            glifo::CAPTURA,
            Accion::Pedido(crate::resultados::pedido_capturar()),
        );
        r.clave = Some("funcion/captura".into());
        vec![r]
    } else {
        vec![Resultado::nuevo(
            format!("Ninguna captura con «{}»", filtro.trim()),
            "Intro: ver todas",
            glifo::AVISO,
            Accion::Consulta(ctx.consulta("capturas ")),
        )]
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn caduca_a_los_siete_dias_de_lo_mas_tarde() {
        assert_eq!(se_va_el(1000, 0, false), Some(1000 + 7 * DIA_MS));
        assert_eq!(se_va_el(1000, 5000, false), Some(5000 + 7 * DIA_MS));
    }

    #[test]
    fn caso_negativo_una_conservada_no_caduca() {
        assert_eq!(se_va_el(1000, 0, true), None);
    }
}
