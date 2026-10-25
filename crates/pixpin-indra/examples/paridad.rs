//! **Paridad con Python**: corre el canal de Rust con los modelos de verdad,
//! servidos por el `onnxruntime` de Python a traves de stdin/stdout (en
//! Linux no hay el ONNX Runtime de Windows que usa `pixpin-ocr`).
//!
//! ```text
//! cargo run --release -p pixpin-indra --example paridad -- \
//!     <carpeta_modelos> <python> <servidor_ort.py> <hilos> <salida> a.pgm b.pgm…
//! ```
//!
//! El servidor es `herramientas/indra/servidor_ort.py` (ver `docs/ocr.md`),
//! en un venv con `onnxruntime`. Escribe `<salida>/<nombre>.txt` con el texto
//! leido y en la consola los milisegundos de cada pagina. `SIN_PREP=1` lee
//! sin preparar la pagina, para comparar con `indra_ocr` tal cual.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use pixpin_indra::canal::{Meta, SalidaOrden, SalidaRec};
use pixpin_indra::lexico::Hunspell;
use pixpin_indra::lm::Lm;
use pixpin_indra::{Gris, Indra, Modelos};

struct Servidor {
    _hijo: Child,
    entrada: ChildStdin,
    salida: BufReader<ChildStdout>,
}

enum Arr {
    F32(Vec<f32>),
    I64(Vec<i64>),
    Bool(Vec<bool>),
}

impl Servidor {
    fn pedir(
        &mut self,
        m: &str,
        entradas: Vec<(&str, Vec<usize>, Arr)>,
    ) -> Vec<(String, Vec<usize>, Arr)> {
        let cab: Vec<serde_json::Value> = entradas
            .iter()
            .map(|(n, f, a)| {
                let t = match a {
                    Arr::F32(_) => "float32",
                    Arr::I64(_) => "int64",
                    Arr::Bool(_) => "bool",
                };
                serde_json::json!([n, t, f])
            })
            .collect();
        let l = serde_json::json!({"m": m, "entradas": cab}).to_string();
        self.entrada.write_all(l.as_bytes()).unwrap();
        self.entrada.write_all(b"\n").unwrap();
        for (_, _, a) in &entradas {
            let b: Vec<u8> = match a {
                Arr::F32(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                Arr::I64(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                Arr::Bool(v) => v.iter().map(|&x| x as u8).collect(),
            };
            self.entrada.write_all(&b).unwrap();
        }
        self.entrada.flush().unwrap();
        let mut linea = String::new();
        self.salida.read_line(&mut linea).unwrap();
        let v: serde_json::Value = serde_json::from_str(&linea).expect("respuesta del servidor");
        let mut out = Vec::new();
        for s in v["salidas"].as_array().unwrap() {
            let nombre = s[0].as_str().unwrap().to_string();
            let tipo = s[1].as_str().unwrap();
            let forma: Vec<usize> = s[2]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect();
            let n: usize = forma.iter().product();
            let a = match tipo {
                "float32" => {
                    let mut b = vec![0u8; n * 4];
                    self.salida.read_exact(&mut b).unwrap();
                    Arr::F32(
                        b.chunks_exact(4)
                            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                            .collect(),
                    )
                }
                "int64" => {
                    let mut b = vec![0u8; n * 8];
                    self.salida.read_exact(&mut b).unwrap();
                    Arr::I64(
                        b.chunks_exact(8)
                            .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
                            .collect(),
                    )
                }
                t => panic!("tipo {t}"),
            };
            out.push((nombre, forma, a));
        }
        out
    }
}

fn f32s(a: Arr) -> Vec<f32> {
    match a {
        Arr::F32(v) => v,
        _ => panic!("se esperaba f32"),
    }
}
fn i64s(a: Arr) -> Vec<i64> {
    match a {
        Arr::I64(v) => v,
        _ => panic!("se esperaba i64"),
    }
}

impl Modelos for Servidor {
    fn detectar(
        &mut self,
        x: Vec<f32>,
        alto: usize,
        ancho: usize,
    ) -> Result<(Vec<f32>, usize, usize), String> {
        let mut o = self.pedir("det", vec![("x", vec![1, 1, alto, ancho], Arr::F32(x))]);
        let (_, f, a) = o.remove(0);
        Ok((f32s(a), f[2], f[3]))
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
        let o = self.pedir(
            "rec",
            vec![
                ("x", vec![lote, 1, alto, ancho], Arr::F32(x)),
                ("widths", vec![lote], Arr::I64(anchos)),
            ],
        );
        let mut r = SalidaRec::default();
        let mut blanco = Vec::new();
        let mut topv = Vec::new();
        let mut topi = Vec::new();
        let mut k = 0;
        for (n, f, a) in o {
            match n.as_str() {
                "ids" => {
                    r.pasos = f[1];
                    r.ids = i64s(a)
                }
                "conf" => r.conf = f32s(a),
                "lengths" => r.longitudes = i64s(a),
                "blank" => blanco = f32s(a),
                "topv" => {
                    k = f[2];
                    topv = f32s(a)
                }
                "topi" => topi = i64s(a),
                _ => {}
            }
        }
        if con_lm && k > 0 {
            r.lm = Some((blanco, topv, topi, k));
        }
        Ok(r)
    }
    fn orientar(&mut self, x: Vec<f32>, lado: usize) -> Result<Option<Vec<f32>>, String> {
        let mut o = self.pedir("orient", vec![("x", vec![1, 1, lado, lado], Arr::F32(x))]);
        Ok(Some(f32s(o.remove(0).2)))
    }
    fn ordenar(&mut self, rasgos: Vec<f32>, n: usize) -> Result<Option<SalidaOrden>, String> {
        let o = self.pedir(
            "order",
            vec![
                ("feats", vec![1, n, 16], Arr::F32(rasgos)),
                ("mask", vec![1, n], Arr::Bool(vec![true; n])),
            ],
        );
        let mut s = SalidaOrden::default();
        for (nombre, f, a) in o {
            match nombre.as_str() {
                "a" => {
                    s.n2 = f[2];
                    s.a = f32s(a)
                }
                "cls" => s.cls = Some(f32s(a)),
                "same" => s.mismo = Some(f32s(a)),
                _ => {}
            }
        }
        Ok(Some(s))
    }
}

/// PGM binario (P5) de 8 bits.
fn leer_pgm(p: &Path) -> Gris {
    let b = std::fs::read(p).unwrap();
    let mut campos = Vec::new();
    let mut i = 0;
    while campos.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ini = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        campos.push(String::from_utf8_lossy(&b[ini..i]).to_string());
    }
    i += 1;
    assert_eq!(campos[0], "P5");
    let (w, h): (usize, usize) = (campos[1].parse().unwrap(), campos[2].parse().unwrap());
    Gris::de_buffer(w, h, b[i..i + w * h].to_vec()).unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(&a[1]);
    let hijo = Command::new(&a[2])
        .args(["-I", &a[3], &a[1], &a[4]])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python");
    let mut hijo = hijo;
    let entrada = hijo.stdin.take().unwrap();
    let salida = BufReader::new(hijo.stdout.take().unwrap());
    let mut srv = Servidor {
        _hijo: hijo,
        entrada,
        salida,
    };
    let t = std::time::Instant::now();
    let meta = Meta::de_json(&std::fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    let lm = Lm::cargar(&dir.join("lm.npz")).ok();
    let mut indra = Indra::nuevo(meta, lm);
    if let (Ok(aff), Ok(dic)) = (
        std::fs::read_to_string(dir.join("es_ES.aff")),
        std::fs::read_to_string(dir.join("es_ES.dic")),
    ) {
        indra.lexico = Some(Hunspell::cargar(&aff, &dic));
    }
    eprintln!(
        "carga {} ms (LM {} MB, lexico {:?} raices)",
        t.elapsed().as_millis(),
        indra.lm.as_ref().map_or(0, |l| l.bytes() / 1_000_000),
        indra.lexico.as_ref().map(|h| h.tamano())
    );
    indra.preparar = std::env::var_os("SIN_PREP").is_none();
    let destino = PathBuf::from(&a[5]);
    std::fs::create_dir_all(&destino).unwrap();
    for f in &a[6..] {
        let g = leer_pgm(Path::new(f));
        let t = std::time::Instant::now();
        let p = indra.leer(&mut srv, &g).unwrap();
        let ms = t.elapsed().as_millis();
        let nombre = Path::new(f)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        std::fs::write(destino.join(format!("{nombre}.txt")), p.texto()).unwrap();
        let mut tiempos: Vec<_> = p.ms.iter().collect();
        tiempos.sort();
        println!(
            "{nombre}: {}x{} {} lineas ({} dudosas) {ms} ms {tiempos:?}",
            g.ancho,
            g.alto,
            p.lineas.len(),
            p.lineas.iter().filter(|l| l.dudosa).count()
        );
    }
}
