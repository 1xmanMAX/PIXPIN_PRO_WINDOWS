//! **Lectura Veloz** (27-oct-2026): la app de lectura rapida del usuario
//! (Tauri, `lecturaveloz://`), en los dos sentidos. El protocolo esta en
//! `docs/integracion-lectura-veloz.md` y lo puro (el enlace de salida, mirar
//! lo que llega, que cambia en los ajustes) en `pixpin_docs::lectura_veloz`;
//! aqui, lo que toca el disco y las ventanas:
//!
//! - **Hacia alli** ([`enviar`]): «⚡ Lectura veloz» del menu de los dos
//!   lectores (PDF y Word/libro). Lo leido se guarda antes, se arma el
//!   enlace con el 🟢 (o por donde se va) y se abre con `ShellExecute`. Si
//!   Lectura Veloz no esta instalada, el lector lo dice en su aviso.
//! - **Desde alli** ([`apuntar`]): el verbo `lectura-veloz` (el enlace
//!   `pixpinmax://lectura-veloz?…`, de fondo mientras se lee alli) mueve el
//!   marcador verde de ESE documento y nada mas: no abre ni trae nada al
//!   frente. Si un lector de ese documento esta abierto, se lo lleva tambien
//!   en memoria ([`tomar`], cada vuelta de su bucle): si no, al cerrarse
//!   guardaria lo suyo encima.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pixpin_docs::lectura::{self, Ajustes};
use pixpin_docs::lectura_veloz::{self as lv, Destino, Progreso, Salida, Tipo};
use pixpin_docs::voz_alta;
use pixpin_motor2d::marcas;

// ------------------------------------------------------------ los abiertos

/// Los documentos con un lector abierto ahora, con su ruta canonica.
static ABIERTOS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Lo que llego de Lectura Veloz para un lector abierto y aun no se tomo.
static PENDIENTES: Mutex<Vec<(PathBuf, Progreso)>> = Mutex::new(Vec::new());

/// La misma ruta escrita de dos maneras (mayusculas, `/`, `\\?\`) es la
/// misma: se compara la canonica.
fn clave(ruta: &Path) -> PathBuf {
    std::fs::canonicalize(ruta).unwrap_or_else(|_| ruta.to_path_buf())
}

/// **Apunta un lector como abierto** mientras viva lo devuelto.
pub struct Abierto(PathBuf);

pub fn abierto(ruta: &Path) -> Abierto {
    let c = clave(ruta);
    ABIERTOS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(c.clone());
    Abierto(c)
}

impl Drop for Abierto {
    fn drop(&mut self) {
        let mut v = ABIERTOS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = v.iter().position(|r| *r == self.0) {
            v.remove(i);
        }
        let queda = v.contains(&self.0);
        drop(v);
        if !queda {
            PENDIENTES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|(r, _)| *r != self.0);
        }
    }
}

fn esta_abierto(c: &Path) -> bool {
    ABIERTOS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|r| r == c)
}

/// **Lo ultimo que llego para el lector de `ruta`**, si llego algo (y se
/// olvida). Lo mira cada lector en cada vuelta de su bucle.
pub fn tomar(ruta: &Path) -> Option<Progreso> {
    let mut v = PENDIENTES.lock().unwrap_or_else(|e| e.into_inner());
    if v.is_empty() {
        return None;
    }
    let c = clave(ruta);
    let mut ultimo = None;
    v.retain(|(r, p)| {
        if *r == c {
            ultimo = Some(p.clone());
            false
        } else {
            true
        }
    });
    ultimo
}

// ------------------------------------------------------------ desde alli

/// **El 🟢 de las marcas de un PDF**, en la pagina y la altura dadas; las
/// demas marcas, como estaban.
pub fn marcas_con_el_verde(marcas_texto: &str, sitio: f64, ahora: i64) -> String {
    let pagina = sitio.floor().max(0.0);
    let (x, y) = marcas::en_la_pagina(pagina as i32, sitio - pagina, 0.5);
    marcas::a_texto(&marcas::con_una_sola(
        &marcas::de_texto(marcas_texto),
        x,
        y,
        voz_alta::EMOJI_DE_VOZ,
        ahora,
    ))
}

/// Lo que cambia en los ajustes de un documento: lo puro
/// (`lectura_veloz::aplicar`) y, por hojas, el 🟢 de sus marcas.
pub fn aplicar(a: &mut Ajustes, p: &Progreso, ahora: i64) {
    lv::aplicar(a, p);
    if p.destino() == Destino::Hojas {
        a.marcas = marcas_con_el_verde(&a.marcas, p.sitio_de_hojas(), ahora);
    }
}

/// **Apunta lo leido en Lectura Veloz** en el documento: su `.pixpin-lectura`
/// y, si es un adjunto del chat, sus `anot-<uid>…` (que viajan al movil),
/// con `anotado_del_adjunto`, que decide cual. Antes, otra vez lo de
/// siempre: un fichero normal que existe, local y de documento. Si su lector
/// esta abierto, ademas se le pasa para que lo lleve en memoria.
///
/// Lo hacen la PixPin que corre (el pedido `lectura_veloz`) o, si no corre,
/// la copia de paso de la linea de ordenes, sin ensenar nada.
pub fn apuntar(p: &Progreso) -> Result<(), String> {
    let ruta = Path::new(&p.ruta);
    if !lv::es_ruta_local(&p.ruta) || !lv::es_documento(ruta) {
        return Err(format!("{} no es un documento local", p.ruta));
    }
    match std::fs::metadata(ruta) {
        Ok(m) if m.is_file() => {}
        Ok(_) => return Err(format!("{} no es un fichero", p.ruta)),
        Err(e) => return Err(format!("{}: {e}", p.ruta)),
    }
    let ahora = crate::lector::ahora_ms() as i64;
    let mut a = crate::anotado_del_adjunto::leer(ruta);
    aplicar(&mut a, p, ahora);
    crate::anotado_del_adjunto::escribir(ruta, &a).map_err(|e| format!("{}: {e}", p.ruta))?;
    let c = clave(ruta);
    if esta_abierto(&c) {
        let mut v = PENDIENTES.lock().unwrap_or_else(|e| e.into_inner());
        v.retain(|(r, _)| *r != c);
        v.push((c, p.clone()));
    }
    tracing::info!(
        ruta = %p.ruta,
        doc = %p.doc,
        pagina = p.pagina,
        fraccion = p.fraccion,
        "lectura veloz: el verde, al dia"
    );
    Ok(())
}

// ------------------------------------------------------------ hacia alli

/// El `doc` de un documento: el uid de su mensaje si es un adjunto del chat;
/// si no, `ruta:` y la huella de su ruta canonica.
pub fn doc_de(ruta: &Path) -> String {
    if let Some((_, x)) = crate::anotado_del_adjunto::adjunto_del_pdf(ruta) {
        return x.uid;
    }
    lv::id_de_ruta(&clave(ruta).to_string_lossy())
}

/// La ruta como la entiende otro programa: absoluta y sin `\\?\`.
fn ruta_para_enviar(ruta: &Path) -> String {
    let r = std::path::absolute(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    let r = r.to_string_lossy();
    r.strip_prefix(r"\\?\").unwrap_or(&r).to_string()
}

/// La fecha de un fichero en milisegundos desde 1970 (0 si no esta).
fn ms_de(f: &Path) -> u64 {
    std::fs::metadata(f)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// **Cuando cambio por ultima vez lo leido** (el sitio, las marcas, el
/// verde): lo mas nuevo de su `.pixpin-lectura` y, de un adjunto, de sus
/// `anot-<uid>.marcas`, `.voz` y `.sitio`. 0 si no hay nada.
pub fn cuando_cambio(ruta: &Path) -> u64 {
    let mut t = ms_de(&lectura::ruta_de_ajustes(ruta));
    if let Some(b) = crate::anotado_del_adjunto::base_de(ruta) {
        for f in [".marcas", ".voz", ".sitio"] {
            t = t.max(ms_de(&b.fichero(f)));
        }
    }
    t
}

/// Guarda los ajustes solo si cambiaron: asi la fecha que se manda ([`cuando_cambio`]) es la
/// del ultimo cambio de verdad, no la de pulsar el boton.
pub fn guardar_si_cambio(ruta: &Path, a: &Ajustes) {
    if crate::anotado_del_adjunto::leer(ruta) == *a {
        return;
    }
    if let Err(err) = crate::anotado_del_adjunto::escribir(ruta, a) {
        tracing::info!(
            ?err,
            "lectura veloz: no se pudo guardar lo leido antes de enviar"
        );
    }
}

/// La [`Salida`] de un documento con su posicion ya calculada.
pub fn salida(
    ruta: &Path,
    tipo: Tipo,
    pagina: u32,
    frac: f32,
    fraccion: f32,
    paginas: u32,
) -> Salida {
    Salida {
        doc: doc_de(ruta),
        ruta: ruta_para_enviar(ruta),
        nombre: pixpin_docs::nombre(ruta),
        tipo,
        pagina,
        frac,
        fraccion,
        paginas,
        t: cuando_cambio(ruta),
    }
}

/// Por que no se pudo abrir Lectura Veloz.
#[derive(Debug)]
pub enum NoSeAbrio {
    /// `lecturaveloz://` no lo tiene nadie: no esta instalada.
    NoInstalada,
    /// Windows no la pudo abrir.
    Fallo(String),
}

impl NoSeAbrio {
    /// El aviso del lector, en su idioma.
    pub fn aviso(&self, textos: &pixpin_store::Catalogo) -> String {
        match self {
            NoSeAbrio::NoInstalada => textos.t("lector-lectura-veloz-no-instalada"),
            NoSeAbrio::Fallo(m) => {
                let mut a = fluent_bundle::FluentArgs::new();
                a.set("motivo", m.clone());
                textos.t_args("lector-lectura-veloz-no-se-pudo", &a)
            }
        }
    }
}

/// **Abre el documento en Lectura Veloz**. Antes de nada se mira que
/// `lecturaveloz://` este registrado: sin el, `ShellExecute` sacaria el
/// «buscar una aplicacion en la Store» de Windows en vez de un fallo.
pub fn enviar(s: &Salida) -> Result<(), NoSeAbrio> {
    if pixpin_shell::orden::orden_del_protocolo(lv::ESQUEMA).is_none() {
        return Err(NoSeAbrio::NoInstalada);
    }
    let enlace = s.enlace();
    tracing::info!(doc = %s.doc, tipo = s.tipo.como_texto(), "lectura veloz: abriendo");
    pixpin_shell::abrir(Path::new(&enlace)).map_err(|e| NoSeAbrio::Fallo(e.message()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-lectura-veloz-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn progreso(ruta: &Path, tipo: &str, pares: &[(&str, &str)]) -> Progreso {
        let mut c: std::collections::BTreeMap<String, String> = pares
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        c.insert("doc".into(), "uid-1".into());
        c.insert("ruta".into(), ruta.to_string_lossy().into_owned());
        c.insert("tipo".into(), tipo.into());
        Progreso::de_campos(&c).unwrap()
    }

    #[test]
    fn el_verde_del_pdf_se_mueve_y_las_demas_marcas_se_quedan() {
        let antes = "1:0.5:2.3:⭐|2:0.5:7.1:🟢";
        let ahora = marcas_con_el_verde(antes, 4.25, 99);
        assert_eq!(ahora, "1:0.5:2.3:⭐|99:0.5:4.25:🟢");
        assert_eq!(marcas_con_el_verde("", 0.0, 5), "5:0.5:0:🟢");
    }

    #[test]
    fn apuntar_escribe_el_verde_junto_al_documento() {
        let d = carpeta("pdf");
        let pdf = d.join("Tesis.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let mut a = Ajustes {
            marcas: "1:0.5:2.3:⭐".into(),
            zoom: 1.5,
            ..Ajustes::default()
        };
        lectura::escribir(&pdf, &a).unwrap();
        apuntar(&progreso(&pdf, "pdf", &[("pagina", "4"), ("frac", "0.25")])).unwrap();
        let leido = lectura::leer(&pdf);
        assert_eq!(leido.pagina, 4.25);
        assert_eq!(leido.zoom, 1.5);
        let m = marcas::de_texto(&leido.marcas);
        assert_eq!(m.len(), 2);
        assert!(m.iter().any(|x| x.emoji == "⭐" && x.y == 2.3));
        assert!(m.iter().any(|x| x.emoji == "🟢" && x.y == 4.25));

        // Un Word: el verde de la voz y el sitio.
        let docx = d.join("Libro.docx");
        std::fs::write(&docx, b"PK").unwrap();
        a = Ajustes::default();
        lectura::escribir(&docx, &a).unwrap();
        apuntar(&progreso(&docx, "texto", &[("fraccion", "0.4")])).unwrap();
        let leido = lectura::leer(&docx);
        assert_eq!(
            (leido.voz, leido.sitio),
            (Some((voz_alta::PARRAFO_DESCONOCIDO, 0.4)), 0.4)
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn caso_negativo_apuntar_no_crea_ni_toca_lo_que_no_es_un_documento() {
        let d = carpeta("malo");
        // Una carpeta con nombre de documento no es un fichero.
        let falsa = d.join("carpeta.pdf");
        std::fs::create_dir_all(&falsa).unwrap();
        assert!(apuntar(&progreso(&falsa, "pdf", &[("pagina", "1")])).is_err());
        // Un documento que no esta: no se crea nada.
        let no_esta = d.join("no-esta.pdf");
        assert!(apuntar(&progreso(&no_esta, "pdf", &[("pagina", "1")])).is_err());
        assert!(!lectura::ruta_de_ajustes(&no_esta).exists());
        assert!(!no_esta.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_lector_abierto_recibe_lo_suyo_y_al_cerrarse_se_olvida() {
        let d = carpeta("abierto");
        let pdf = d.join("Abierto.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let p = progreso(&pdf, "pdf", &[("pagina", "2")]);
        {
            let _a = abierto(&pdf);
            apuntar(&p).unwrap();
            apuntar(&progreso(&pdf, "pdf", &[("pagina", "3")])).unwrap();
            // Solo lo ultimo.
            assert_eq!(tomar(&pdf).map(|x| x.pagina), Some(3));
            assert_eq!(tomar(&pdf), None);
            apuntar(&p).unwrap();
        }
        // Cerrado: lo que quedaba se olvida, y lo nuevo ya no se encola.
        assert_eq!(tomar(&pdf), None);
        apuntar(&p).unwrap();
        assert_eq!(tomar(&pdf), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
