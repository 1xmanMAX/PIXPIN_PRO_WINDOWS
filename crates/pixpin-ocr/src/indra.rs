//! **El lector Indra en Windows**: los cuatro modelos de `pixpin-indra` con
//! el ONNX Runtime de Windows (`pixpin_voz::ort`, el mismo enlace que usa
//! Whisper), su descarga y su memoria.
//!
//! - **Runtime**: el `onnxruntime.dll` de `System32` (Windows 11 24H2 en
//!   adelante: version 1.17 o mas nueva, sobra para los modelos de Indra,
//!   ONNX IR 8 / opset 17). En los Windows que no lo traen, [`bajar`] baja
//!   el oficial 1.31.0 (MIT) a `<datos>/whisper/onnxruntime.dll` —el sitio
//!   que ya mira Whisper, que asi tambien funciona—, solo la DLL del ZIP
//!   (6 MB de 83) y comprobada con su sha256.
//! - **Pesos**: a `<datos>/indra/`, los mismos ficheros que el escaner,
//!   comprobados con su sha256 ([`pixpin_indra::catalogo`]). Solo si el
//!   usuario lo pide en Ajustes, por la licencia (ver `docs/ocr.md`).
//! - **Memoria**: se carga al leer la primera imagen (~0,5 s) y se suelta a
//!   los tres minutos sin leer nada: unos 150-250 MB mientras esta, nada
//!   despues. El equipo de 4 GB no lo paga mientras no se usa.
//!
//! Este modulo habla con una libreria en C: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pixpin_geom::Rect;
use pixpin_indra::canal::{Meta, SalidaOrden, SalidaRec};
use pixpin_indra::catalogo::{FICHEROS, Fichero, RUNTIME, datos_de_la_entrada};
use pixpin_indra::lexico::{Hunspell, Vocabulario};
use pixpin_indra::lm::Lm;
use pixpin_indra::{Gris, Indra, Modelos};
use pixpin_voz::ErrorVoz;
use pixpin_voz::ort::{Runtime, Sesion, Valor};
use pixpin_voz::whisper::bajar as red;
use sha2::{Digest, Sha256};

use crate::{Config, ErrorOcr, Linea, Palabra};

/// Sin leer nada en este tiempo, se suelta (modelos, LM y diccionario).
const SOLTAR_TRAS: Duration = Duration::from_secs(180);

/// `<datos>/indra/`.
pub fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("indra")
}

fn esta_entero(p: &Path, f: &Fichero) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.len() == f.bytes)
}

/// Si los pesos estan bajados (con su tamano; el sha256 se comprobo al
/// bajarlos). No mira el runtime: eso es [`runtime_listo`].
pub fn listo(raiz: &Path) -> bool {
    let c = carpeta(raiz);
    FICHEROS.iter().all(|f| esta_entero(&c.join(f.nombre), f))
}

/// Si hay un ONNX Runtime que cargar (el de Windows o uno bajado).
pub fn runtime_listo(raiz: &Path, junto_al_exe: Option<&Path>) -> bool {
    pixpin_voz::ort::buscar(raiz, junto_al_exe).is_some()
}

/// Lo que habria que bajar, en MB (pesos y, si falta, el runtime).
pub fn megas_a_bajar(raiz: &Path, junto_al_exe: Option<&Path>) -> u64 {
    let mut b: u64 = if listo(raiz) {
        0
    } else {
        FICHEROS.iter().map(|f| f.bytes).sum()
    };
    if !runtime_listo(raiz, junto_al_exe) {
        b += RUNTIME.largo;
    }
    b.div_ceil(1_000_000)
}

fn de_voz(e: ErrorVoz) -> ErrorOcr {
    match e {
        ErrorVoz::Cancelada => ErrorOcr::Cancelada,
        ErrorVoz::Descarga { que, detalle } => ErrorOcr::Descarga { que, detalle },
        otro => ErrorOcr::Indra(otro.to_string()),
    }
}

fn fallo(que: &str, detalle: impl std::fmt::Display) -> ErrorOcr {
    ErrorOcr::Descarga {
        que: que.to_string(),
        detalle: detalle.to_string(),
    }
}

/// sha256 de un fichero, en hexadecimal.
pub fn sha256_de(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// **Baja el lector Indra** (y el ONNX Runtime si este Windows no lo trae),
/// en el hilo de quien llama. `avance` de 0 a 1. Lo ya bajado no se vuelve
/// a bajar; un fichero que no da su sha256 se borra y es un error.
pub fn bajar(
    raiz: &Path,
    junto_al_exe: Option<&Path>,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<(), ErrorOcr> {
    let destino = carpeta(raiz);
    std::fs::create_dir_all(&destino).map_err(|e| fallo(&destino.display().to_string(), e))?;
    let falta_runtime = !runtime_listo(raiz, junto_al_exe);
    let total: u64 = FICHEROS.iter().map(|f| f.bytes).sum::<u64>()
        + if falta_runtime { RUNTIME.largo } else { 0 };
    let mut hecho: u64 = 0;
    for f in FICHEROS {
        let p = destino.join(f.nombre);
        if !esta_entero(&p, &f) {
            let antes = hecho;
            red::bajar(
                f.url,
                &p,
                &mut |leidos, _| {
                    avance(((antes + leidos.min(f.bytes)) as f32 / total as f32).min(0.99))
                },
                cancelado,
            )
            .map_err(de_voz)?;
            let sha = sha256_de(&p).map_err(|e| fallo(f.nombre, e))?;
            if sha != f.sha256 {
                let _ = std::fs::remove_file(&p);
                return Err(fallo(f.nombre, format!("el sha256 no cuadra ({sha})")));
            }
        }
        hecho += f.bytes;
    }
    if falta_runtime {
        let antes = hecho;
        bajar_runtime(
            raiz,
            &mut |b| avance(((antes + b) as f32 / total as f32).min(0.99)),
            cancelado,
        )?;
    }
    avance(1.0);
    tracing::info!(carpeta = %destino.display(), runtime_bajado = falta_runtime, "lector Indra bajado");
    Ok(())
}

/// Solo la entrada de `onnxruntime.dll` del ZIP oficial, descomprimida y
/// comprobada, a `<datos>/whisper/onnxruntime.dll`.
fn bajar_runtime(
    raiz: &Path,
    avance: &mut dyn FnMut(u64),
    cancelado: &AtomicBool,
) -> Result<(), ErrorOcr> {
    let dir = raiz.join("whisper");
    std::fs::create_dir_all(&dir).map_err(|e| fallo("onnxruntime.dll", e))?;
    let tramo = dir.join("onnxruntime.zip-tramo");
    red::bajar_tramo(
        RUNTIME.url,
        &tramo,
        RUNTIME.desde,
        RUNTIME.largo,
        &mut |leidos, _| avance(leidos),
        cancelado,
    )
    .map_err(de_voz)?;
    let datos = std::fs::read(&tramo).map_err(|e| fallo("onnxruntime.dll", e));
    let _ = std::fs::remove_file(&tramo);
    let datos = datos?;
    let ini = datos_de_la_entrada(&datos)
        .ok_or_else(|| fallo("onnxruntime.dll", "el tramo no es la entrada del ZIP"))?;
    let fin = (ini as u64 + RUNTIME.comprimido) as usize;
    let comprimido = datos
        .get(ini..fin)
        .ok_or_else(|| fallo("onnxruntime.dll", "tramo corto"))?;
    let mut dll = Vec::with_capacity(RUNTIME.bytes as usize);
    flate2::read::DeflateDecoder::new(comprimido)
        .read_to_end(&mut dll)
        .map_err(|e| fallo("onnxruntime.dll", e))?;
    let sha = hex(&Sha256::digest(&dll));
    if dll.len() as u64 != RUNTIME.bytes || sha != RUNTIME.sha256 {
        return Err(fallo(
            "onnxruntime.dll",
            format!("no es la DLL esperada ({} bytes, {sha})", dll.len()),
        ));
    }
    let destino = dir.join(pixpin_voz::ort::NOMBRE_DE_LA_DLL);
    let parte = red::ruta_parte(&destino);
    std::fs::write(&parte, &dll).map_err(|e| fallo("onnxruntime.dll", e))?;
    let _ = std::fs::remove_file(&destino);
    std::fs::rename(&parte, &destino).map_err(|e| fallo("onnxruntime.dll", e))?;
    Ok(())
}

/// Borra lo bajado (Ajustes › «Borrar Indra»). El runtime bajado se queda:
/// puede estar usandolo Whisper.
pub fn borrar(raiz: &Path) -> std::io::Result<()> {
    soltar();
    let c = carpeta(raiz);
    if c.exists() {
        std::fs::remove_dir_all(c)?;
    }
    Ok(())
}

/// Los cuatro modelos abiertos.
struct Sesiones {
    det: Sesion<'static>,
    rec: Sesion<'static>,
    orden: Option<Sesion<'static>>,
    orient: Option<Sesion<'static>>,
    rt: &'static Runtime,
}

fn onnx(e: ErrorVoz) -> String {
    e.to_string()
}

impl Sesiones {
    fn salida<'a>(
        s: &Sesion<'_>,
        salidas: &'a [Valor<'static>],
        nombre: &str,
    ) -> Option<&'a Valor<'static>> {
        s.salidas
            .iter()
            .position(|n| n.to_bytes() == nombre.as_bytes())
            .and_then(|i| salidas.get(i))
    }
}

impl Modelos for Sesiones {
    fn detectar(
        &mut self,
        x: Vec<f32>,
        alto: usize,
        ancho: usize,
    ) -> Result<(Vec<f32>, usize, usize), String> {
        let t = self
            .rt
            .tensor_f32(x, &[1, 1, alto as i64, ancho as i64])
            .map_err(onnx)?;
        let o = self.det.correr(&[&t]).map_err(onnx)?;
        let s = o.first().ok_or("det.onnx sin salida")?;
        let f = s.forma().map_err(onnx)?;
        if f.len() != 4 {
            return Err(format!("det.onnx dio la forma {f:?}"));
        }
        Ok((
            s.como_f32().map_err(onnx)?.to_vec(),
            f[2] as usize,
            f[3] as usize,
        ))
    }

    fn reconocer(
        &mut self,
        x: Vec<f32>,
        lote: usize,
        alto: usize,
        ancho: usize,
        anchos: Vec<i64>,
        con_lm: bool,
    ) -> Result<SalidaRec, String> {
        let tx = self
            .rt
            .tensor_f32(x, &[lote as i64, 1, alto as i64, ancho as i64])
            .map_err(onnx)?;
        let tw = self.rt.tensor_i64(anchos, &[lote as i64]).map_err(onnx)?;
        let o = self.rec.correr(&[&tx, &tw]).map_err(onnx)?;
        let toma =
            |n: &str| Self::salida(&self.rec, &o, n).ok_or_else(|| format!("rec.onnx sin «{n}»"));
        let ids = toma("ids")?;
        let pasos = ids.forma().map_err(onnx)?.get(1).copied().unwrap_or(0) as usize;
        let mut r = SalidaRec {
            ids: ids.como_i64().map_err(onnx)?.to_vec(),
            conf: toma("conf")?.como_f32().map_err(onnx)?.to_vec(),
            longitudes: toma("lengths")?.como_i64().map_err(onnx)?.to_vec(),
            pasos,
            lm: None,
        };
        if con_lm && let (Ok(b), Ok(tv), Ok(ti)) = (toma("blank"), toma("topv"), toma("topi")) {
            let k = tv.forma().map_err(onnx)?.get(2).copied().unwrap_or(0) as usize;
            if k > 0 {
                r.lm = Some((
                    b.como_f32().map_err(onnx)?.to_vec(),
                    tv.como_f32().map_err(onnx)?.to_vec(),
                    ti.como_i64().map_err(onnx)?.to_vec(),
                    k,
                ));
            }
        }
        Ok(r)
    }

    fn orientar(&mut self, x: Vec<f32>, lado: usize) -> Result<Option<Vec<f32>>, String> {
        let Some(s) = &self.orient else {
            return Ok(None);
        };
        let t = self
            .rt
            .tensor_f32(x, &[1, 1, lado as i64, lado as i64])
            .map_err(onnx)?;
        let o = s.correr(&[&t]).map_err(onnx)?;
        Ok(Some(
            o.first()
                .ok_or("orient.onnx sin salida")?
                .como_f32()
                .map_err(onnx)?
                .to_vec(),
        ))
    }

    fn ordenar(&mut self, rasgos: Vec<f32>, n: usize) -> Result<Option<SalidaOrden>, String> {
        let Some(s) = &self.orden else {
            return Ok(None);
        };
        let tf = self
            .rt
            .tensor_f32(rasgos, &[1, n as i64, 16])
            .map_err(onnx)?;
        let tm = self
            .rt
            .tensor_bool(vec![true; n], &[1, n as i64])
            .map_err(onnx)?;
        let o = s.correr(&[&tf, &tm]).map_err(onnx)?;
        let a = Self::salida(s, &o, "a").ok_or("order.onnx sin «a»")?;
        let n2 = a.forma().map_err(onnx)?.get(2).copied().unwrap_or(n as i64) as usize;
        let mut r = SalidaOrden {
            a: a.como_f32().map_err(onnx)?.to_vec(),
            n2,
            cls: None,
            mismo: None,
        };
        if let (Some(c), Some(m)) = (Self::salida(s, &o, "cls"), Self::salida(s, &o, "same")) {
            r.cls = Some(c.como_f32().map_err(onnx)?.to_vec());
            r.mismo = Some(m.como_f32().map_err(onnx)?.to_vec());
        }
        Ok(Some(r))
    }
}

/// El lector cargado: el canal, los modelos y con que se cargo.
struct Cargado {
    indra: Indra,
    sesiones: Sesiones,
    ultima: Instant,
}

// SAFETY: `Sesiones` guarda punteros de ONNX Runtime (`OrtSession`,
// `OrtValue` temporales) que no son de ningun hilo: la documentacion de
// ONNX Runtime permite usar una sesion desde cualquier hilo, y aqui solo
// se toca con el cerrojo de `CARGADO` cogido, de uno en uno.
unsafe impl Send for Cargado {}

static CARGADO: Mutex<Option<Cargado>> = Mutex::new(None);
static VIGILANTE: AtomicBool = AtomicBool::new(false);

/// Suelta el lector si estaba cargado (cambio de ajustes, borrar, poca
/// memoria). Se vuelve a cargar solo al leer.
pub fn soltar() {
    if let Ok(mut g) = CARGADO.lock()
        && g.take().is_some()
    {
        tracing::info!("lector Indra soltado");
    }
}

/// Si el lector esta cargado ahora mismo (sin esperar a quien lee: si el
/// cerrojo esta cogido, alguien lo esta usando y por tanto esta cargado).
pub fn cargado() -> bool {
    match CARGADO.try_lock() {
        Ok(g) => g.is_some(),
        Err(std::sync::TryLockError::WouldBlock) => true,
        Err(std::sync::TryLockError::Poisoned(_)) => false,
    }
}

/// **Carga el lector por adelantado**, para que la primera lectura no pague
/// el ~0,5-1 s de abrir los modelos. Devuelve si lo cargo ahora.
///
/// No le pelea al vigilante de los tres minutos: si ya estaba cargado no
/// toca su `ultima` (precargar no es leer y no alarga su vida), y lo recien
/// cargado cuenta desde ahora, asi que si nadie lee se suelta igual. Si el
/// cerrojo esta cogido es que alguien esta leyendo (y cargando): no se le
/// espera.
pub fn precargar(c: &Config) -> Result<bool, ErrorOcr> {
    let raiz = c
        .raiz
        .as_deref()
        .ok_or_else(|| ErrorOcr::IndraNoListo("sin carpeta de datos".into()))?;
    let mut guarda = match CARGADO.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err(ErrorOcr::Indra("cerrojo envenenado".into()));
        }
    };
    if guarda.is_some() {
        return Ok(false);
    }
    *guarda = Some(cargar(c, raiz)?);
    vigilar();
    Ok(true)
}

fn cargar(c: &Config, raiz: &Path) -> Result<Cargado, ErrorOcr> {
    let t = Instant::now();
    let dir = carpeta(raiz);
    let rt = Runtime::del_proceso(raiz, c.junto_al_exe.as_deref())
        .map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))?;
    let hilos = c.hilos.clamp(1, 8);
    pixpin_indra::gris::fijar_hilos(hilos);
    let abre = |n: &str| rt.abrir_con(&dir.join(n), hilos, false);
    let opcional = |n: &str| dir.join(n).is_file().then(|| abre(n)).transpose();
    let sesiones = Sesiones {
        det: abre("det.onnx").map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))?,
        rec: abre("rec.onnx").map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))?,
        orden: opcional("order.onnx").map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))?,
        orient: opcional("orient.onnx").map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))?,
        rt,
    };
    let meta = std::fs::read_to_string(dir.join("meta.json"))
        .map_err(|e| ErrorOcr::IndraNoListo(e.to_string()))
        .and_then(|t| Meta::de_json(&t).map_err(ErrorOcr::IndraNoListo))?;
    // Sin las salidas de la busqueda en haz (un rec.onnx viejo) no hay LM.
    let con_salidas_lm = ["blank", "topv", "topi"].iter().all(|n| {
        sesiones
            .rec
            .salidas
            .iter()
            .any(|s| s.to_bytes() == n.as_bytes())
    });
    let lm = if con_salidas_lm {
        Lm::cargar(&dir.join("lm.npz"))
            .map_err(|e| tracing::warn!(error = %e, "Indra sin modelo de lenguaje"))
            .ok()
    } else {
        None
    };
    let mut indra = Indra::nuevo(meta, lm);
    if c.corregir
        && let (Ok(aff), Ok(dic)) = (
            std::fs::read_to_string(dir.join("es_ES.aff")),
            std::fs::read_to_string(dir.join("es_ES.dic")),
        )
    {
        indra.lexico = Some(Hunspell::cargar(&aff, &dic));
    }
    indra.vocabulario = Vocabulario::de_texto(&c.vocabulario);
    tracing::info!(
        ms = t.elapsed().as_millis() as u64,
        runtime = %rt.version,
        dll = %rt.dll.display(),
        lm_mb = indra.lm.as_ref().map_or(0, |l| l.bytes() / 1_000_000),
        lexico = indra.lexico.as_ref().map_or(0, |h| h.tamano()),
        hilos,
        "lector Indra cargado"
    );
    Ok(Cargado {
        indra,
        sesiones,
        ultima: Instant::now(),
    })
}

/// Un hilo que suelta el lector cuando lleva [`SOLTAR_TRAS`] sin leer.
fn vigilar() {
    if VIGILANTE.swap(true, Ordering::SeqCst) {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("indra-soltar".into())
        .spawn(|| {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                if let Ok(mut g) = CARGADO.lock()
                    && g.as_ref().is_some_and(|c| c.ultima.elapsed() > SOLTAR_TRAS)
                {
                    *g = None;
                    tracing::info!("lector Indra soltado por no usarse");
                }
            }
        });
    if lanzado.is_err() {
        VIGILANTE.store(false, Ordering::SeqCst);
    }
}

/// La caja entera que cubre `[x0, y0, x1, y1]`.
pub fn rect_de(b: &[f32; 4]) -> Rect {
    let x0 = b[0].floor() as i32;
    let y0 = b[1].floor() as i32;
    let x1 = b[2].ceil() as i32;
    let y1 = b[3].ceil() as i32;
    Rect {
        x: x0,
        y: y0,
        ancho: (x1 - x0).max(0) as u32,
        alto: (y1 - y0).max(0) as u32,
    }
}

/// Las lineas de Indra en la forma de siempre: en orden de lectura, sin las
/// vacias ni las dudosas.
pub fn a_lineas(p: &pixpin_indra::Pagina) -> Vec<Linea> {
    p.lineas
        .iter()
        .filter(|l| !l.dudosa && !l.texto.trim().is_empty())
        .map(|l| Linea {
            caja: rect_de(&l.caja),
            texto: l.texto.clone(),
            palabras: l
                .palabras
                .iter()
                .map(|w| Palabra {
                    caja: rect_de(&w.caja),
                    texto: w.texto.clone(),
                })
                .collect(),
        })
        .collect()
}

/// **Lee una imagen RGBA con Indra.**
pub fn reconocer(c: &Config, ancho: u32, alto: u32, rgba: &[u8]) -> Result<Vec<Linea>, ErrorOcr> {
    let raiz = c
        .raiz
        .as_deref()
        .ok_or_else(|| ErrorOcr::IndraNoListo("sin carpeta de datos".into()))?;
    let g = Gris::de_rgba(ancho as usize, alto as usize, rgba).ok_or(ErrorOcr::ImagenInvalida)?;
    // Mas de 4000 px de lado no gana nada (el detector trabaja a 1024).
    let lado = g.ancho.max(g.alto);
    let g = if lado > pixpin_indra::canal::LADO_MAX {
        let s = pixpin_indra::canal::LADO_MAX as f64 / lado as f64;
        pixpin_indra::gris::redimensionar(
            &g,
            ((g.ancho as f64 * s).round() as usize).max(1),
            ((g.alto as f64 * s).round() as usize).max(1),
            pixpin_indra::gris::Interpolacion::Area,
        )
    } else {
        g
    };
    let mut guarda = CARGADO
        .lock()
        .map_err(|_| ErrorOcr::Indra("cerrojo envenenado".into()))?;
    if guarda.is_none() {
        *guarda = Some(cargar(c, raiz)?);
        vigilar();
    }
    let Some(cargado) = guarda.as_mut() else {
        return Err(ErrorOcr::IndraNoListo("no se pudo cargar".into()));
    };
    let t = Instant::now();
    let mut p = cargado
        .indra
        .leer(&mut cargado.sesiones, &g)
        .map_err(ErrorOcr::Indra)?;
    cargado.ultima = Instant::now();
    drop(guarda);
    p.a_la_imagen(
        ancho as usize,
        alto as usize,
        ancho as f32 / g.ancho as f32,
        alto as f32 / g.alto as f32,
    );
    let lineas = a_lineas(&p);
    tracing::info!(
        ms = t.elapsed().as_millis() as u64,
        ancho,
        alto,
        lineas = lineas.len(),
        dudosas = p.lineas.iter().filter(|l| l.dudosa).count(),
        giro = p.giro,
        inclinacion = p.inclinacion,
        "Indra leyo una imagen"
    );
    Ok(lineas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-ocr-indra-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sin_ficheros_no_esta_listo_y_con_un_tamano_malo_tampoco() {
        let r = temporal("listo");
        assert!(!listo(&r));
        let c = carpeta(&r);
        std::fs::create_dir_all(&c).unwrap();
        for f in FICHEROS {
            std::fs::write(c.join(f.nombre), vec![0u8; f.bytes as usize]).unwrap();
        }
        assert!(listo(&r));
        std::fs::write(c.join("rec.onnx"), b"corto").unwrap();
        assert!(!listo(&r), "un fichero a medias no vale");
        borrar(&r).unwrap();
        assert!(!c.exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn el_sha256_de_un_fichero_es_el_de_siempre() {
        let r = temporal("sha");
        let p = r.join("abc.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_de(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn las_lineas_dudosas_y_vacias_no_salen_y_las_cajas_cubren() {
        use pixpin_indra::canal::{Linea as L, Pagina, Palabra as P};
        let l = |t: &str, dudosa: bool| L {
            caja: [10.2, 5.7, 50.1, 20.0],
            texto: t.into(),
            confianza: 0.9,
            palabras: vec![P {
                texto: t.into(),
                caja: [10.2, 5.7, 50.1, 20.0],
                confianza: 0.9,
                probs: None,
            }],
            dudosa,
        };
        let p = Pagina {
            lineas: vec![l("Hola", false), l("14,0", true), l("", false)],
            bloques: vec![],
            giro: 0,
            inclinacion: 0.0,
            ancho: 100,
            alto: 100,
            ms: Default::default(),
        };
        let v = a_lineas(&p);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].texto, "Hola");
        assert_eq!(
            v[0].caja,
            Rect {
                x: 10,
                y: 5,
                ancho: 41,
                alto: 15
            }
        );
        assert_eq!(v[0].palabras.len(), 1);
    }

    #[test]
    fn lo_que_falta_por_bajar_se_cuenta_en_megas() {
        let r = temporal("megas");
        let m = megas_a_bajar(&r, None);
        // 26 de pesos, y 7 mas si este Windows no trae el runtime.
        assert!(m == 26 || m == 33, "{m}");
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Con los pesos de verdad en `PIXPIN_INDRA_PRUEBA` (una carpeta de
    /// datos con `indra/` dentro) y un runtime: lee una pagina pintada.
    #[test]
    #[ignore = "necesita los pesos bajados y ONNX Runtime; ejecutar con --ignored"]
    fn lee_con_los_pesos_de_verdad() {
        let Some(raiz) = std::env::var_os("PIXPIN_INDRA_PRUEBA").map(PathBuf::from) else {
            return;
        };
        let c = Config {
            motor: crate::Motor::Indra,
            raiz: Some(raiz),
            hilos: 2,
            corregir: true,
            ..Config::default()
        };
        let (w, h) = (400u32, 100u32);
        let rgba = vec![255u8; (w * h * 4) as usize];
        let l = reconocer(&c, w, h, &rgba).unwrap();
        assert!(l.is_empty(), "una pagina en blanco no tiene texto");
    }
}
