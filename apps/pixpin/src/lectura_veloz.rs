//! **Lectura Veloz** (27-oct-2026): la app de lectura rapida del usuario
//! (Tauri, `lecturaveloz://`), en los dos sentidos. El protocolo esta en
//! `docs/integracion-lectura-veloz.md` y lo puro (el enlace de salida, mirar
//! lo que llega, que cambia en los ajustes) en `pixpin_docs::lectura_veloz`;
//! aqui, lo que toca el disco y las ventanas:
//!
//! - **Hacia alli** ([`enviar`]): «⚡ Lectura veloz» del menu del mensaje
//!   del chat ([`abrir_guardado`]) y del ⋮ de la nota ([`abrir_nota`]);
//!   desde el 29-oct-2026 ya no del ⋮ de los lectores (como el movil: «se
//!   abre solo desde el menu del mensaje»). Lo leido se
//!   guarda antes, se arma el enlace con el 🟢 (o por donde se va) y se abre
//!   con `ShellExecute`. Si no se puede, el aviso dice por que de verdad
//!   (sin codigo, formato que no lee, falta el archivo, no instalada, el
//!   fallo de Windows) y el registro lo apunta.
//! - **Lo ultimo suyo** ([`porcentaje`]): cada punto que llega se guarda
//!   tambien en este equipo, por documento (`lectura-veloz\ultimos.txt`,
//!   no viaja): es la chapa «Lectura veloz · N %» de la burbuja y el % junto
//!   a la opcion.
//! - **Desde alli** ([`apuntar`]): el verbo `lectura-veloz` (el enlace
//!   `pixpinmax://lectura-veloz?…`, de fondo mientras se lee alli) mueve el
//!   marcador verde de ESE documento y nada mas: no abre ni trae nada al
//!   frente. Si un lector de ese documento esta abierto, se lo lleva tambien
//!   en memoria ([`tomar`], cada vuelta de su bucle): si no, al cerrarse
//!   guardaria lo suyo encima.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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
    recordar(p);
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

/// **Por que no se pudo abrir Lectura Veloz** (`lv::Motivo`), con su aviso.
#[derive(Debug)]
pub struct NoSeAbrio(pub lv::Motivo);

impl NoSeAbrio {
    /// El aviso del lector o del chat, en su idioma.
    pub fn aviso(&self, textos: &pixpin_store::Catalogo) -> String {
        aviso(textos, &self.0)
    }
}

/// **El aviso de un motivo**: lo que falla de verdad, no un «no se pudo».
pub fn aviso(textos: &pixpin_store::Catalogo, m: &lv::Motivo) -> String {
    let con = |clave: &str, nombre: &str, valor: &str| {
        let mut a = fluent_bundle::FluentArgs::new();
        a.set(nombre, valor.to_string());
        textos.t_args(clave, &a)
    };
    match m {
        lv::Motivo::NoInstalada => textos.t("lector-lectura-veloz-no-instalada"),
        lv::Motivo::SinCodigo => textos.t("lector-lectura-veloz-sin-codigo"),
        lv::Motivo::Formato(e) => con("lector-lectura-veloz-formato", "ext", e),
        lv::Motivo::FaltaArchivo(n) => con("lector-lectura-veloz-falta-archivo", "nombre", n),
        lv::Motivo::Fallo(x) => con("lector-lectura-veloz-no-se-pudo", "motivo", x),
    }
}

/// Lo apunta en el registro (como `PixPinLecturaVeloz` del movil: un
/// proximo «no se abre» del usuario se mira ahi) y lo devuelve.
fn no_se_abrio(doc: &str, ruta: &Path, m: lv::Motivo) -> NoSeAbrio {
    tracing::warn!(
        doc,
        ruta = %ruta.display(),
        motivo = %m.para_el_registro(),
        "lectura veloz: no se abrio"
    );
    NoSeAbrio(m)
}

/// **Abre el documento en Lectura Veloz**. Antes de nada se mira que
/// `lecturaveloz://` este registrado: sin el, `ShellExecute` sacaria el
/// «buscar una aplicacion en la Store» de Windows en vez de un fallo.
pub fn enviar(s: &Salida) -> Result<(), NoSeAbrio> {
    let ruta = Path::new(&s.ruta);
    if pixpin_shell::orden::orden_del_protocolo(lv::ESQUEMA).is_none() {
        return Err(no_se_abrio(&s.doc, ruta, lv::Motivo::NoInstalada));
    }
    let enlace = s.enlace();
    tracing::info!(doc = %s.doc, tipo = s.tipo.como_texto(), "lectura veloz: abriendo");
    pixpin_shell::abrir(Path::new(&enlace)).map_err(|e| {
        let m = lv::motivo_de_error("abrir Lectura Veloz", &e.message());
        no_se_abrio(&s.doc, ruta, lv::Motivo::Fallo(m))
    })
}

/// **«⚡ Lectura veloz» sin lector abierto** (el menu del mensaje del chat,
/// la nota): desde lo guardado del documento, el 🟢 si lo hay (en un PDF,
/// el de sus marcas) y si no por donde se iba. `doc` es el codigo del
/// mensaje; sin el, o si Lectura Veloz no lee ese formato, o si el archivo
/// ya no esta, se dice cual de las tres (`lv::motivo_del_documento`).
pub fn abrir_guardado(ruta: &Path, doc: Option<&str>) -> Result<(), NoSeAbrio> {
    if let Some(m) = lv::motivo_del_documento(doc, ruta, ruta.is_file()) {
        return Err(no_se_abrio(doc.unwrap_or_default(), ruta, m));
    }
    let a = crate::anotado_del_adjunto::leer(ruta);
    let tipo = lv::tipo_de(ruta).unwrap_or(Tipo::Texto);
    let mut s = match tipo {
        Tipo::Pdf => {
            let donde = marcas::de_texto(&a.marcas)
                .into_iter()
                .find(|m| m.emoji == voz_alta::EMOJI_DE_VOZ)
                .map_or(a.pagina, |m| m.y);
            // Sin el PDF abierto no se sabe cuantas hojas tiene: 0, «no se
            // sabe», como dice el protocolo.
            let (pagina, frac, fraccion) = lv::posicion_de_hojas(donde, 0);
            salida(ruta, tipo, pagina, frac, fraccion, 0)
        }
        Tipo::Texto => {
            let donde = a.voz.map_or(a.sitio, |(_, f)| f);
            salida(ruta, tipo, 0, 0.0, donde, 0)
        }
    };
    s.doc = doc.unwrap_or_default().to_string();
    enviar(&s)
}

/// **Una nota del chat en Lectura Veloz** (el ⋮ de la nota, como
/// `deNota` del movil): su texto se escribe como `.md` en la carpeta de
/// este equipo (`lectura-veloz/notas/<huella del codigo>/`), y su `doc` es
/// el codigo de la nota. Lo de antes de esa nota se quita, salvo su
/// `.pixpin-lectura` (por donde se iba).
pub fn abrir_nota(codigo: Option<&str>, titulo: &str, texto: &str) -> Result<(), NoSeAbrio> {
    let Some(codigo) = codigo.filter(|c| lv::doc_valido(c)) else {
        return Err(no_se_abrio("", Path::new(titulo), lv::Motivo::SinCodigo));
    };
    let preparar = || -> std::io::Result<PathBuf> {
        let huella = lv::id_de_ruta(codigo);
        let huella = huella.trim_start_matches(lv::PREFIJO_RUTA);
        let carpeta = carpeta_local()
            .ok_or_else(|| std::io::Error::other("no hay carpeta de datos"))?
            .join("notas")
            .join(huella);
        std::fs::create_dir_all(&carpeta)?;
        let nombre = format!("{}.md", nombre_de_nota(titulo));
        let ruta = carpeta.join(&nombre);
        let ajustes = lectura::ruta_de_ajustes(&ruta);
        for f in std::fs::read_dir(&carpeta)?.flatten() {
            let p = f.path();
            if p != ruta && p != ajustes && p.is_file() {
                let _ = std::fs::remove_file(p);
            }
        }
        std::fs::write(&ruta, texto)?;
        Ok(ruta)
    };
    match preparar() {
        Ok(ruta) => abrir_guardado(&ruta, Some(codigo)),
        Err(e) => {
            let m = lv::motivo_de_error("preparar la nota", &e.to_string());
            Err(no_se_abrio(codigo, Path::new(titulo), lv::Motivo::Fallo(m)))
        }
    }
}

/// El nombre de fichero de una nota: su titulo sin lo que Windows no deja
/// en un nombre, corto, y «Nota» si no queda nada.
fn nombre_de_nota(titulo: &str) -> String {
    let limpio: String = titulo
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " ()._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let limpio = limpio.trim().trim_end_matches('.').trim();
    if limpio.is_empty() {
        "Nota".to_string()
    } else {
        limpio.to_string()
    }
}

// ------------------------------------------------------------ lo ultimo suyo

/// Lo ultimo de cada documento, leido una vez del disco ([`lv::Ultimos`]).
static ULTIMOS: Mutex<Option<lv::Ultimos>> = Mutex::new(None);

/// Sube cada vez que cambia lo ultimo de algun documento: el chat vuelve a
/// medir sus burbujas (la chapa puede salir o crecer).
static GENERACION: AtomicU64 = AtomicU64::new(0);

/// El `doc` de cada ruta ya mirada: la nota de un fichero pregunta su %
/// y buscar el mensaje de un adjunto recorre el chat.
static DOCS: Mutex<BTreeMap<PathBuf, String>> = Mutex::new(BTreeMap::new());

/// **La carpeta de este equipo** (`<datos>\lectura-veloz`): lo que no viaja.
#[cfg(not(test))]
fn carpeta_local() -> Option<PathBuf> {
    let u = crate::oscuro::ubicacion().or_else(|| {
        let dir = pixpin_shell::entorno::directorio_del_ejecutable().ok()?;
        let appdata = pixpin_shell::entorno::appdata().ok()?;
        Some(pixpin_store::rutas::resolver(&dir, &appdata))
    })?;
    Some(u.raiz().join("lectura-veloz"))
}

#[cfg(test)]
fn carpeta_local() -> Option<PathBuf> {
    Some(std::env::temp_dir().join(format!("pixpin-lectura-veloz-local-{}", std::process::id())))
}

fn fichero_de_ultimos() -> Option<PathBuf> {
    Some(carpeta_local()?.join("ultimos.txt"))
}

fn leer_ultimos() -> lv::Ultimos {
    fichero_de_ultimos()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| lv::Ultimos::de_texto(&t))
        .unwrap_or_default()
}

/// **Apunta lo que llego** como lo ultimo de su documento, si es lo mas
/// nuevo. Se relee el fichero antes: la copia de paso de la linea de
/// ordenes tambien escribe en el.
fn recordar(p: &Progreso) {
    let u = lv::Ultimo::de_progreso(p, crate::lector::ahora_ms());
    let mut g = ULTIMOS.lock().unwrap_or_else(|e| e.into_inner());
    let mut todos = leer_ultimos();
    if todos.con(&p.doc, u)
        && let Some(f) = fichero_de_ultimos()
    {
        let hecho = f
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&f, todos.a_texto()));
        if let Err(err) = hecho {
            tracing::info!(?err, "lectura veloz: no se pudo apuntar lo ultimo");
        }
    }
    *g = Some(todos);
    drop(g);
    GENERACION.fetch_add(1, Ordering::Relaxed);
}

/// **El % de lo leido en Lectura Veloz** de un documento, si alguna vez lo
/// mando.
pub fn porcentaje(doc: &str) -> Option<u8> {
    let mut g = ULTIMOS.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(leer_ultimos)
        .de(doc)
        .map(lv::Ultimo::porcentaje)
}

/// El % de un documento por su ruta ([`doc_de`], recordado).
pub fn porcentaje_de_ruta(ruta: &Path) -> Option<u8> {
    let doc = {
        let mut docs = DOCS.lock().unwrap_or_else(|e| e.into_inner());
        docs.entry(ruta.to_path_buf())
            .or_insert_with(|| doc_de(ruta))
            .clone()
    };
    porcentaje(&doc)
}

/// Cuantas veces cambio lo ultimo de algun documento.
pub fn generacion() -> u64 {
    GENERACION.load(Ordering::Relaxed)
}

/// **«⚡ Lectura veloz»**, con el % al lado si ya se leyo alli.
pub fn rotulo(textos: &pixpin_store::Catalogo, pct: Option<u8>) -> String {
    match pct {
        None => textos.t("lector-lectura-veloz"),
        Some(n) => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("pct", n);
            textos.t_args("lector-lectura-veloz-con", &a)
        }
    }
}

/// **La chapa de la burbuja**: «Lectura veloz · 47 %».
pub fn chapa(textos: &pixpin_store::Catalogo, pct: u8) -> String {
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("pct", pct);
    textos.t_args("chat-lectura-veloz-chapa", &a)
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
    fn lo_que_llega_queda_como_lo_ultimo_y_da_el_porcentaje() {
        let d = carpeta("ultimo");
        let pdf = d.join("Chapa.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let mut p = progreso(&pdf, "pdf", &[("pagina", "4"), ("fraccion", "0.47")]);
        p.doc = "uid-chapa".into();
        p.t = 2_000;
        let antes = generacion();
        apuntar(&p).unwrap();
        assert_eq!(porcentaje("uid-chapa"), Some(47));
        assert!(generacion() > antes);
        // Uno mas viejo que llega tarde no hace retroceder la chapa.
        let viejo = Progreso {
            t: 1_000,
            fraccion: 0.1,
            ..p.clone()
        };
        apuntar(&viejo).unwrap();
        assert_eq!(porcentaje("uid-chapa"), Some(47));
        // Y queda en el disco de este equipo, para la proxima vez.
        assert_eq!(
            leer_ultimos().de("uid-chapa").map(lv::Ultimo::porcentaje),
            Some(47)
        );
        assert_eq!(porcentaje("uid-que-nunca-se-leyo"), None);
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(chapa(&textos, 47), "Lectura veloz · 47 %");
        assert_eq!(rotulo(&textos, Some(47)), "⚡ Lectura veloz · 47 %");
        assert_eq!(rotulo(&textos, None), "⚡ Lectura veloz");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn caso_negativo_sin_codigo_ni_archivo_dice_por_que_no_se_abre() {
        let d = carpeta("motivos");
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let no_esta = d.join("Se fue.pdf");
        let Err(no) = abrir_guardado(&no_esta, Some("uid-1")) else {
            panic!("se abrio un archivo que no esta");
        };
        assert_eq!(no.0, lv::Motivo::FaltaArchivo("Se fue.pdf".into()));
        assert!(no.aviso(&textos).contains("Se fue.pdf"));
        let foto = d.join("foto.png");
        std::fs::write(&foto, b"png").unwrap();
        let Err(no) = abrir_guardado(&foto, Some("uid-1")) else {
            panic!("se abrio una foto");
        };
        assert!(
            no.aviso(&textos).contains("«.png»"),
            "{}",
            no.aviso(&textos)
        );
        let Err(no) = abrir_nota(None, "Nueva", "# Hola") else {
            panic!("se abrio una nota sin guardar");
        };
        assert_eq!(no.0, lv::Motivo::SinCodigo);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_nombre_de_la_nota_vale_para_un_fichero() {
        assert_eq!(nombre_de_nota("Clase: tema 3/4"), "Clase_ tema 3_4");
        assert_eq!(nombre_de_nota("  ...  "), "Nota");
        assert_eq!(nombre_de_nota(""), "Nota");
        assert_eq!(nombre_de_nota(&"a".repeat(200)).len(), 80);
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
