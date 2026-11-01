//! **Modelo de lenguaje n-grama de Indra (`lm.npz`) y busqueda en haz CTC**
//! para las lineas dudosas (`indra_ocr/lm.py`, `IndraLm.kt`).
//!
//! `lm.npz` es un ZIP de `.npy`: por longitud de historia `n`, las claves
//! ordenadas de los pares (historia, ficha) con su log-probabilidad
//! (`k<n>`, `v<n>`) y las claves de las historias con su peso de retroceso
//! (`hk<n>`, `hb<n>`). Una clave empaqueta fichas de `bits` bits (0 = inicio
//! de linea, `t + 1` = ficha `t`).
//!
//! Banco del escaner: sin el LM, el manuscrito ingles (GNHK) baja de 49,7 a
//! 31,4 % de palabras.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::vocab::Vocab;

/// Tope del cache de log-probabilidades (Python usa un millon; demasiado
/// para un equipo de 4 GB).
const CACHE_MAX: usize = 200_000;

pub struct Lm {
    pub orden: usize,
    pub tam_vocab: usize,
    pub bits: u32,
    k: Vec<Vec<u64>>,
    v: Vec<Vec<f32>>,
    hk: Vec<Vec<u64>>,
    hb: Vec<Vec<f32>>,
    cache: HashMap<u64, f64>,
    /// Cota de cualquier `logp_clave` (poda de la busqueda).
    pub max_logp: f64,
}

/// Un `.npy` leido: el tipo y los bytes en little-endian.
#[derive(Debug)]
pub struct Npy {
    pub tipo: String,
    pub cuenta: usize,
    pub datos: Vec<u8>,
}

impl Npy {
    /// `.npy` v1/v2/v3, little-endian, orden C.
    pub fn leer(b: &[u8]) -> Result<Npy, String> {
        if b.len() < 10 || b[0] != 0x93 || &b[1..6] != b"NUMPY" {
            return Err("no es .npy".into());
        }
        let mayor = b[6];
        let (largo, ini) = if mayor == 1 {
            (u16::from_le_bytes([b[8], b[9]]) as usize, 10)
        } else {
            if b.len() < 12 {
                return Err("cabecera .npy corta".into());
            }
            (u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize, 12)
        };
        let cab = std::str::from_utf8(b.get(ini..ini + largo).ok_or("cabecera .npy corta")?)
            .map_err(|_| "cabecera .npy no es texto")?;
        let tipo = entre(cab, "'descr':", "'").ok_or("npy sin descr")?;
        if !(tipo.starts_with('<') || tipo.starts_with('|')) {
            return Err(format!("npy {tipo}: solo little-endian"));
        }
        if cab.contains("'fortran_order': True") {
            return Err("npy en orden Fortran".into());
        }
        let forma = cab
            .split("'shape':")
            .nth(1)
            .and_then(|r| r.split('(').nth(1))
            .and_then(|r| r.split(')').next())
            .ok_or("npy sin shape")?;
        let mut cuenta: usize = 1;
        for d in forma.split(',').map(str::trim).filter(|d| !d.is_empty()) {
            cuenta = cuenta.saturating_mul(d.parse::<usize>().map_err(|_| "npy: forma rara")?);
        }
        let tam: usize = tipo[2..].parse().map_err(|_| "npy: tipo raro")?;
        let datos = b
            .get(ini + largo..ini + largo + cuenta * tam)
            .ok_or("npy cortado")?
            .to_vec();
        Ok(Npy {
            tipo,
            cuenta,
            datos,
        })
    }

    pub fn entero(&self, i: usize) -> Result<i64, String> {
        match &self.tipo[1..] {
            "u8" | "i8" => Ok(i64::from_le_bytes(
                self.datos[i * 8..i * 8 + 8].try_into().unwrap_or_default(),
            )),
            "i4" | "u4" => Ok(i32::from_le_bytes(
                self.datos[i * 4..i * 4 + 4].try_into().unwrap_or_default(),
            ) as i64),
            t => Err(format!("npy {t} no es entero")),
        }
    }

    pub fn u64s(&self) -> Vec<u64> {
        self.datos
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap_or_default()))
            .collect()
    }

    pub fn f32s(&self) -> Result<Vec<f32>, String> {
        match &self.tipo[1..] {
            "f4" => Ok(self
                .datos
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect()),
            "f8" => Ok(self
                .datos
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or_default()) as f32)
                .collect()),
            t => Err(format!("npy {t} no es real")),
        }
    }
}

/// El valor entre comillas que sigue a `clave` en la cabecera de un `.npy`.
fn entre(cab: &str, clave: &str, comilla: &str) -> Option<String> {
    let r = cab.split(clave).nth(1)?;
    let r = r.split(comilla).nth(1)?;
    Some(r.to_string())
}

pub fn lse(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let m = a.max(b);
    m + ((a - m).exp() + (b - m).exp()).ln()
}

/// Sesgo por palabras en la busqueda en haz (vocabulario del usuario y
/// lexico): una bonificacion que depende solo del prefijo —lo de las
/// palabras ya cerradas mas lo de la palabra a medias— y que se suma al LM
/// mientras crece el haz. Una palabra del vocabulario que el reconocedor ve
/// posible gana a una lectura parecida; un prefijo que no llega a palabra
/// devuelve su bonificacion al cerrarse. Palabras = tramos de letras y
/// cifras, en minusculas.
pub trait SesgoDePalabras {
    fn activo(&self) -> bool;
    /// Bonificacion de la palabra a medio escribir.
    fn parcial(&self, w: &str) -> f64;
    /// Bonificacion de la palabra ya cerrada.
    fn completa(&self, w: &str) -> f64;
    /// Cota de lo que puede sumar un paso (poda).
    fn paso_max(&self) -> f64;
}

#[derive(Clone)]
struct Haz {
    b: f64,
    nb: f64,
    s: f64,
    pasos: Vec<u32>,
    hk: u64,
}

/// La salida del reconocedor que necesita la busqueda, de una linea.
pub struct SalidaDeLinea<'a> {
    /// log-prob del blanco por paso `[T]`.
    pub blanco: &'a [f32],
    /// Las `k` mejores clases por paso y su log-prob (`[T · k]`).
    pub topv: &'a [f32],
    pub topi: &'a [i64],
    pub k: usize,
    pub pasos: usize,
}

impl Lm {
    pub fn cargar(fichero: &Path) -> Result<Lm, String> {
        let f = std::fs::File::open(fichero).map_err(|e| e.to_string())?;
        Lm::de_zip(f)
    }

    pub fn de_zip<R: std::io::Read + std::io::Seek>(r: R) -> Result<Lm, String> {
        let mut z = zip::ZipArchive::new(r).map_err(|e| e.to_string())?;
        let mut arr: HashMap<String, Npy> = HashMap::new();
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| e.to_string())?;
            let Some(nombre) = e.name().strip_suffix(".npy").map(str::to_string) else {
                continue;
            };
            let mut b = Vec::with_capacity(e.size() as usize);
            e.read_to_end(&mut b).map_err(|e| e.to_string())?;
            arr.insert(nombre, Npy::leer(&b)?);
        }
        let toma = |n: &str| arr.get(n).ok_or_else(|| format!("lm.npz sin {n}"));
        let orden = toma("order")?.entero(0)? as usize;
        let tam_vocab = toma("vocab_size")?.entero(0)? as usize;
        let bits = match arr.get("bits") {
            Some(b) => b.entero(0)? as u32,
            None => 11,
        };
        if orden == 0 || orden as u32 * bits > 61 {
            return Err(format!("orden {orden} x {bits} bits no caben en una clave"));
        }
        let mut k = Vec::new();
        let mut v = Vec::new();
        let mut hk = Vec::new();
        let mut hb = Vec::new();
        for i in 0..orden {
            k.push(toma(&format!("k{i}"))?.u64s());
            v.push(toma(&format!("v{i}"))?.f32s()?);
            hk.push(toma(&format!("hk{i}"))?.u64s());
            hb.push(toma(&format!("hb{i}"))?.f32s()?);
        }
        let mut tope = -(tam_vocab as f64).ln();
        for a in &v {
            for &x in a {
                tope = tope.max(x as f64);
            }
        }
        let mut retro = 0.0;
        for a in &hb {
            if let Some(m) = a.iter().copied().reduce(f32::max) {
                retro += (m as f64).max(0.0);
            }
        }
        Ok(Lm {
            orden,
            tam_vocab,
            bits,
            k,
            v,
            hk,
            hb,
            cache: HashMap::new(),
            max_logp: tope + retro,
        })
    }

    /// Lo que ocupa en memoria (para el registro).
    pub fn bytes(&self) -> usize {
        self.k.iter().map(|a| a.len() * 8).sum::<usize>()
            + self.v.iter().map(|a| a.len() * 4).sum::<usize>()
            + self.hk.iter().map(|a| a.len() * 8).sum::<usize>()
            + self.hb.iter().map(|a| a.len() * 4).sum::<usize>()
    }

    /// log P(ficha | historia) para una clave = las ultimas `orden − 1`
    /// fichas de la historia y luego la ficha, empaquetadas (las historias
    /// cortas son los bits bajos). Lo mismo que `NgramLM.logp_keys`.
    pub fn logp_clave(&mut self, clave: u64) -> f64 {
        if let Some(&v) = self.cache.get(&clave) {
            return v;
        }
        let b = self.bits;
        let ct = clave & ((1u64 << b) - 1);
        let hist = clave >> b;
        let n = self.orden;
        let mut h_ok = vec![false; n];
        let mut h_v = vec![0.0; n];
        let mut p_ok = vec![false; n];
        let mut p_v = vec![0.0; n];
        for i in 0..n {
            let mascara = if i == 0 {
                0
            } else {
                (1u64 << (b * i as u32)) - 1
            };
            let hq = hist & mascara;
            if let Ok(j) = self.hk[i].binary_search(&hq) {
                h_ok[i] = true;
                h_v[i] = self.hb[i][j] as f64;
            }
            if let Ok(j) = self.k[i].binary_search(&((hq << b) | ct)) {
                p_ok[i] = true;
                p_v[i] = self.v[i][j] as f64;
            }
        }
        h_ok[0] = true;
        let mut m = 0;
        for &ok in h_ok.iter().skip(1) {
            if ok {
                m += 1;
            } else {
                break;
            }
        }
        let elegido = (0..=m).rev().find(|&i| p_ok[i]);
        let r = match elegido {
            Some(c) => h_v[c + 1..=m].iter().sum::<f64>() + p_v[c],
            None => h_v[..=m].iter().sum::<f64>() - (self.tam_vocab as f64).ln(),
        };
        if self.cache.len() < CACHE_MAX {
            self.cache.insert(clave, r);
        }
        r
    }

    /// log P de una tira de clases desde el inicio de linea.
    pub fn logp_de(&mut self, clases: &[u32]) -> f64 {
        let b = self.bits;
        let mascara = (1u64 << (b * (self.orden as u32 - 1))) - 1;
        let mut hist = 0u64;
        let mut s = 0.0;
        for &c in clases {
            let clave = (hist << b) | (c as u64 + 1);
            s += self.logp_clave(clave);
            hist = clave & mascara;
        }
        s
    }

    /// Busqueda en haz CTC por prefijos (`beam_search` de `lm.py`).
    /// Devuelve (clases, paso en que salio cada una).
    #[allow(clippy::too_many_arguments)]
    pub fn buscar_en_haz(
        &mut self,
        e: &SalidaDeLinea<'_>,
        alfa: f64,
        beta: f64,
        haz: usize,
        salto_blanco: f64,
        suelo: f64,
        sesgo: Option<(&dyn SesgoDePalabras, &Vocab)>,
    ) -> (Vec<u32>, Vec<u32>) {
        let sesgo = sesgo.filter(|(s, _)| s.activo());
        let b = self.bits;
        let mascara = (1u64 << (b * (self.orden as u32 - 1))) - 1;
        let mut haces: Vec<(Vec<u32>, Haz)> = vec![(
            Vec::new(),
            Haz {
                b: 0.0,
                nb: f64::NEG_INFINITY,
                s: 0.0,
                pasos: Vec::new(),
                hk: 0,
            },
        )];
        let log_salto = salto_blanco.ln();
        let log_suelo = suelo.ln();
        let alcance = beta + alfa * self.max_logp + sesgo.map_or(0.0, |(s, _)| s.paso_max());
        for t in 0..e.pasos {
            let b0 = e.blanco[t] as f64;
            if b0 > log_salto {
                for (_, h) in &mut haces {
                    h.b = lse(h.b, h.nb) + b0;
                    h.nb = f64::NEG_INFINITY;
                }
                continue;
            }
            // Siguiente generacion, con el orden de insercion (como el
            // LinkedHashMap del puerto de Kotlin).
            let mut sig: Vec<(Vec<u32>, Haz)> = Vec::new();
            let mut pos: HashMap<Vec<u32>, usize> = HashMap::new();
            let mut cand: Vec<(u32, f64)> = Vec::with_capacity(e.k);
            for j in 0..e.k {
                let c = e.topi[t * e.k + j];
                let lv = e.topv[t * e.k + j] as f64;
                if c != 0 && lv >= log_suelo {
                    cand.push((c as u32, lv));
                }
            }
            for (p, h) in &haces {
                anadir(
                    &mut sig,
                    &mut pos,
                    p.clone(),
                    lse(h.b, h.nb) + b0,
                    f64::NEG_INFINITY,
                    h,
                );
                let ultimo = p.last().copied();
                for &(c, lv) in &cand {
                    if Some(c) == ultimo {
                        anadir(
                            &mut sig,
                            &mut pos,
                            p.clone(),
                            f64::NEG_INFINITY,
                            h.nb + lv,
                            h,
                        );
                    }
                }
            }
            let piso = if haces.len() >= haz {
                sig.iter()
                    .map(|(_, h)| lse(h.b, h.nb) + h.s)
                    .fold(f64::INFINITY, f64::min)
            } else {
                f64::NEG_INFINITY
            };
            let mut extender: Vec<(usize, u32, f64)> = Vec::new();
            for (ih, (p, h)) in haces.iter().enumerate() {
                let total = lse(h.b, h.nb);
                let ultimo = p.last().copied();
                for &(c, lv) in &cand {
                    let nuevo = if Some(c) == ultimo {
                        h.b + lv
                    } else {
                        total + lv
                    };
                    if nuevo == f64::NEG_INFINITY {
                        continue;
                    }
                    let mut q = p.clone();
                    q.push(c);
                    if pos.contains_key(&q) {
                        anadir(&mut sig, &mut pos, q, f64::NEG_INFINITY, nuevo, h);
                    } else if nuevo + h.s + alcance >= piso - 1e-9 {
                        extender.push((ih, c, nuevo));
                    }
                }
            }
            for (ih, c, nuevo) in extender {
                let (p, h) = &haces[ih];
                let clave = (h.hk << b) | (c as u64 + 1);
                let lmp = self.logp_clave(clave);
                let wd = sesgo.map_or(0.0, |(s, v)| delta_de_sesgo(s, v, p, c));
                let mut q = p.clone();
                q.push(c);
                let mut pasos = h.pasos.clone();
                pasos.push(t as u32);
                let nuevo_haz = Haz {
                    b: f64::NEG_INFINITY,
                    nb: nuevo,
                    s: h.s + alfa * lmp + beta + wd,
                    pasos,
                    hk: clave & mascara,
                };
                anadir_haz(&mut sig, &mut pos, q, nuevo_haz);
            }
            // sortedByDescending es estable: en empate, el orden de entrada.
            let mut idx: Vec<usize> = (0..sig.len()).collect();
            let nota = |h: &Haz| lse(h.b, h.nb) + h.s;
            idx.sort_by(|&x, &y| nota(&sig[y].1).total_cmp(&nota(&sig[x].1)));
            idx.truncate(haz);
            let mut nuevos = Vec::with_capacity(idx.len());
            let mut sig: Vec<Option<(Vec<u32>, Haz)>> = sig.into_iter().map(Some).collect();
            for i in idx {
                if let Some(x) = sig[i].take() {
                    nuevos.push(x);
                }
            }
            haces = nuevos;
        }
        let mut mejor: Option<&(Vec<u32>, Haz)> = None;
        let mut nota_mejor = f64::NEG_INFINITY;
        for x in &haces {
            let mut sc = lse(x.1.b, x.1.nb) + x.1.s;
            if let Some((s, v)) = sesgo {
                // Fin de linea: la palabra en curso se cierra.
                let w = palabra_final(v, &x.0);
                if !w.is_empty() {
                    sc += s.completa(&w) - s.parcial(&w);
                }
            }
            if mejor.is_none() || sc > nota_mejor {
                mejor = Some(x);
                nota_mejor = sc;
            }
        }
        mejor.map_or((Vec::new(), Vec::new()), |(p, h)| {
            (p.clone(), h.pasos.clone())
        })
    }
}

/// Junta `(b, nb)` en el prefijo `p` si ya esta en la generacion (se queda
/// con su `s`, sus pasos y su historia), o lo anade con los de `h`.
fn anadir(
    sig: &mut Vec<(Vec<u32>, Haz)>,
    pos: &mut HashMap<Vec<u32>, usize>,
    p: Vec<u32>,
    bb: f64,
    nbb: f64,
    h: &Haz,
) {
    anadir_haz(
        sig,
        pos,
        p,
        Haz {
            b: bb,
            nb: nbb,
            s: h.s,
            pasos: h.pasos.clone(),
            hk: h.hk,
        },
    );
}

fn anadir_haz(
    sig: &mut Vec<(Vec<u32>, Haz)>,
    pos: &mut HashMap<Vec<u32>, usize>,
    p: Vec<u32>,
    h: Haz,
) {
    if let Some(&i) = pos.get(&p) {
        let o = &mut sig[i].1;
        o.b = lse(o.b, h.b);
        o.nb = lse(o.nb, h.nb);
    } else {
        pos.insert(p.clone(), sig.len());
        sig.push((p, h));
    }
}

/// La letra (o cifra), en minuscula, de una clase; `None` si no es de palabra.
fn letra_de(v: &Vocab, clase: u32) -> Option<String> {
    let c = v.ficha(clase)?;
    let mut it = c.chars();
    let ch = it.next()?;
    (it.next().is_none() && ch.is_alphanumeric()).then(|| ch.to_lowercase().collect())
}

/// La palabra en curso al final del prefijo (minusculas).
fn palabra_final(v: &Vocab, prefijo: &[u32]) -> String {
    let mut i = prefijo.len();
    while i > 0 && letra_de(v, prefijo[i - 1]).is_some() {
        i -= 1;
    }
    prefijo[i..]
        .iter()
        .filter_map(|&c| letra_de(v, c))
        .collect()
}

fn delta_de_sesgo(s: &dyn SesgoDePalabras, v: &Vocab, prefijo: &[u32], c: u32) -> f64 {
    let w = palabra_final(v, prefijo);
    match letra_de(v, c) {
        Some(ch) => s.parcial(&format!("{w}{ch}")) - if w.is_empty() { 0.0 } else { s.parcial(&w) },
        None if w.is_empty() => 0.0,
        None => s.completa(&w) - s.parcial(&w),
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use std::io::Write;

    pub fn npy(tipo: &str, forma: &str, datos: &[u8]) -> Vec<u8> {
        let mut cab = format!("{{'descr': '{tipo}', 'fortran_order': False, 'shape': {forma}, }}");
        while (10 + cab.len() + 1) % 64 != 0 {
            cab.push(' ');
        }
        cab.push('\n');
        let mut o = vec![0x93];
        o.extend_from_slice(b"NUMPY");
        o.extend_from_slice(&[1, 0]);
        o.extend_from_slice(&(cab.len() as u16).to_le_bytes());
        o.extend_from_slice(cab.as_bytes());
        o.extend_from_slice(datos);
        o
    }

    fn u64s(v: &[u64]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }
    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// LM de orden 2 con 11 bits y dos fichas (IndraUnitTest).
    pub fn lm_de_prueba() -> Lm {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let op = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let mut pon = |n: &str, t: &str, f: &str, d: Vec<u8>| {
                z.start_file(format!("{n}.npy"), op).unwrap();
                z.write_all(&npy(t, f, &d)).unwrap();
            };
            pon("order", "<i8", "()", u64s(&[2]));
            pon("vocab_size", "<i8", "()", u64s(&[2]));
            pon("k0", "<u8", "(2,)", u64s(&[1, 2]));
            pon("v0", "<f4", "(2,)", f32s(&[-1.0, -2.0]));
            pon("hk0", "<u8", "(1,)", u64s(&[0]));
            pon("hb0", "<f4", "(1,)", f32s(&[0.0]));
            pon("k1", "<u8", "(1,)", u64s(&[(1 << 11) | 2]));
            pon("v1", "<f4", "(1,)", f32s(&[-0.1]));
            pon("hk1", "<u8", "(2,)", u64s(&[0, 1]));
            pon("hb1", "<f4", "(2,)", f32s(&[-0.5, -0.7]));
            z.finish().unwrap();
        }
        buf.set_position(0);
        Lm::de_zip(buf).unwrap()
    }

    #[test]
    fn lector_npy_y_modelo_de_lenguaje() {
        let mut lm = lm_de_prueba();
        assert_eq!(lm.orden, 2);
        assert!((lm.logp_clave((1 << 11) | 2) - -0.1).abs() < 1e-6); // bigrama visto
        assert!((lm.logp_clave((1 << 11) | 1) - (-0.7 - 1.0)).abs() < 1e-6); // retroceso
        assert!((lm.logp_clave(2) - (-0.5 - 2.0)).abs() < 1e-6); // inicio de linea
        // Busqueda en haz: un candidato claro por paso.
        let ln = |p: f64| p.ln() as f32;
        let blanco = [ln(0.01), ln(0.99), ln(0.01)];
        let topv = [ln(0.98), ln(0.01), ln(0.99), ln(0.005), ln(0.98), ln(0.01)];
        let topi = [1, 0, 0, 2, 2, 0];
        let e = SalidaDeLinea {
            blanco: &blanco,
            topv: &topv,
            topi: &topi,
            k: 2,
            pasos: 3,
        };
        let (ids, pasos) = lm.buscar_en_haz(&e, 0.5, 1.5, 8, 0.999, 1e-3, None);
        assert_eq!(ids, [1, 2]);
        assert_eq!(pasos.len(), 2);
        assert_eq!(pasos[0], 0);
    }

    #[test]
    fn un_npy_roto_se_dice() {
        assert!(Npy::leer(b"nada").is_err());
        let b = npy("<f4", "(3,)", &[0; 4]);
        assert!(Npy::leer(&b).is_err()); // faltan 8 bytes
        let b = npy(">f4", "(1,)", &[0; 4]);
        assert!(Npy::leer(&b).is_err());
    }
}
