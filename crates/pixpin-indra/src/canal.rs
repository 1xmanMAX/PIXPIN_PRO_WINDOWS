//! **La pagina entera** (`IndraOcr.kt` / `pipeline.py`): enderezar, detectar
//! los renglones, leerlos, cortar donde el detector junto dos columnas,
//! ordenar y agrupar en bloques. Los cuatro modelos van detras de
//! [`Modelos`]: aqui no hay ONNX Runtime, y por eso todo esto se prueba con
//! modelos de mentira.
//!
//! Solo el reconocedor latino (`rec.onnx`, que lee tambien manuscrito
//! latino, cirilico y griego): sin los de CJK, arabe e indicos, sin
//! matematicas y sin markdown, como el escaner. La app quiere lineas,
//! palabras y bloques con sus cajas.

use std::collections::HashMap;

use crate::corrector::{Corrector, Desempate};
use crate::deteccion;
use crate::dudas;
use crate::gris::{Gris, rot90};
use crate::imagen::{self, Caja, INCLINACION_MIN};
use crate::lexico::{Hunspell, Vocabulario};
use crate::lm::{Lm, SalidaDeLinea};
use crate::orden::{self, Bloque};
use crate::vocab::Vocab;

pub const ALTO_MIN_ADAPTATIVO: f32 = 9.0;
pub const ALTO_OBJETIVO_ADAPTATIVO: f64 = 12.0;
pub const LADO_MAX_ADAPTATIVO: usize = 2048;
/// Cortar un renglon detectado donde dos letras vecinas estan a mas de
/// tantas alturas (el detector junto dos celdas o columnas).
pub const HUECO_CORTE: f32 = 3.0;
pub const HUECO_CALLE: f32 = 1.6;
pub const BORDES_CALLE: usize = 3;
/// El escaner gira con 0,9 (Python 0,95): en su banco de 132 paginas, a
/// 0,9 giran 4 y todas bien; a 0,8 entra un giro malo.
pub const ORIENTACION_MIN: f32 = 0.9;
/// Bajo esta confianza la linea no da texto (casillas, rayas)...
pub const CONFIANZA_MIN: f32 = 0.5;
/// ...pero desde esta se conserva como dudosa si no es basura.
pub const SUELO_DUDA: f32 = 0.3;
pub const PUERTA_LM: f64 = 0.9;
pub const ALFA_LM: f64 = 0.5;
pub const BETA_LM: f64 = 1.5;
pub const HAZ_LM: usize = 8;
pub const SUELO_CANDIDATO_LM: f64 = 3e-3;
pub const HUECO_MARCADOR: f32 = 2.5;
const RELLENO: &str = ".·…‥_-–—=";
const GUIONES: &str = "-–—";
const VINETAS: &str = "•◦▪▫●○■□‣⁃∙*";
/// Lado largo maximo que recibe Indra: el detector trabaja a 1024 y los
/// recortes no ganan nada pasado esto.
pub const LADO_MAX: usize = 4000;

/// Lo que necesita el LM de una salida de `rec.onnx`: log-prob del blanco
/// `[lote · pasos]`, las `k` mejores `[lote · pasos · k]`, sus clases y `k`.
pub type SalidaLm = (Vec<f32>, Vec<f32>, Vec<i64>, usize);

/// Lo que da `rec.onnx` para un lote.
#[derive(Debug, Clone, Default)]
pub struct SalidaRec {
    /// Clase mas probable por paso `[lote · pasos]`.
    pub ids: Vec<i64>,
    /// Su probabilidad `[lote · pasos]`.
    pub conf: Vec<f32>,
    /// Pasos validos de cada linea `[lote]`.
    pub longitudes: Vec<i64>,
    pub pasos: usize,
    /// Para el LM: log-prob del blanco `[lote · pasos]`, las `k` mejores
    /// `[lote · pasos · k]` y sus clases.
    pub lm: Option<SalidaLm>,
}

/// Lo que da `order.onnx`.
#[derive(Debug, Clone, Default)]
pub struct SalidaOrden {
    /// Logits por pares `[n · n2]` (n2 = n).
    pub a: Vec<f32>,
    pub n2: usize,
    /// Logits de clase de bloque `[n · 10]`.
    pub cls: Option<Vec<f32>>,
    /// «Mismo bloque» `[n · n2]`.
    pub mismo: Option<Vec<f32>>,
}

/// Quien corre los modelos. En Windows, `pixpin-ocr` con el ONNX Runtime
/// del sistema.
pub trait Modelos {
    /// `det.onnx` sobre `[1, 1, alto, ancho]`: los planos de salida y su
    /// alto y ancho (`[canales, alto_s, ancho_s]`).
    fn detectar(
        &mut self,
        x: Vec<f32>,
        alto: usize,
        ancho: usize,
    ) -> Result<(Vec<f32>, usize, usize), String>;
    /// `rec.onnx` sobre `[lote, 1, alto, ancho]` con el ancho util de cada
    /// linea; `con_lm` pide tambien las salidas de la busqueda en haz.
    fn reconocer(
        &mut self,
        x: Vec<f32>,
        lote: usize,
        alto: usize,
        ancho: usize,
        anchos: Vec<i64>,
        con_lm: bool,
    ) -> Result<SalidaRec, String>;
    /// `orient.onnx` sobre `[1, 1, lado, lado]`: las 5 probabilidades
    /// (0, 90, 180, 270 antihorario, vertical), o `None` sin el modelo.
    fn orientar(&mut self, x: Vec<f32>, lado: usize) -> Result<Option<Vec<f32>>, String>;
    /// `order.onnx` sobre `[1, n, 16]`, o `None` sin el modelo.
    fn ordenar(&mut self, rasgos: Vec<f32>, n: usize) -> Result<Option<SalidaOrden>, String>;
}

/// Una palabra con su caja y su confianza (media de sus letras).
#[derive(Debug, Clone, PartialEq)]
pub struct Palabra {
    pub texto: String,
    pub caja: Caja,
    pub confianza: f32,
    /// La probabilidad de cada letra, si hay una por caracter.
    pub probs: Option<Vec<f32>>,
}

/// Una linea en orden de lectura.
#[derive(Debug, Clone, PartialEq)]
pub struct Linea {
    pub caja: Caja,
    pub texto: String,
    pub confianza: f32,
    pub palabras: Vec<Palabra>,
    /// Confianza entre [`SUELO_DUDA`] y [`CONFIANZA_MIN`]: su lectura se
    /// conserva, marcada, pero no va al texto plano.
    pub dudosa: bool,
}

/// La pagina leida. Cajas en pixeles de la pagina GIRADA `giro` grados
/// (antihorario: se giro en sentido horario para ponerla derecha), ya sin
/// la inclinacion pequena; `ancho` x `alto` = la pagina girada.
#[derive(Debug, Clone, PartialEq)]
pub struct Pagina {
    pub lineas: Vec<Linea>,
    pub bloques: Vec<Bloque>,
    pub giro: i32,
    pub inclinacion: f64,
    pub ancho: usize,
    pub alto: usize,
    pub ms: HashMap<&'static str, u64>,
}

impl Pagina {
    /// El texto plano: las lineas seguras, una por renglon.
    pub fn texto(&self) -> String {
        self.lineas
            .iter()
            .filter(|l| !l.dudosa && !l.texto.is_empty())
            .map(|l| l.texto.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Las cajas de vuelta a la imagen que entro (`IndraResult.kt`): se
    /// deshace el giro de cuartos de vuelta y se escala por `sx`, `sy`
    /// (pixeles de la imagen por pixel de lo que leyo Indra). La pagina
    /// cambia en el sitio; devuelve el angulo de las lineas (grados,
    /// horario positivo).
    pub fn a_la_imagen(&mut self, ancho_img: usize, alto_img: usize, sx: f32, sy: f32) -> f32 {
        let k = (self.giro / 90).rem_euclid(4);
        let (w0, h0) = if k % 2 == 1 {
            (self.alto as f32, self.ancho as f32)
        } else {
            (self.ancho as f32, self.alto as f32)
        };
        let (aw, ah) = (ancho_img as f32, alto_img as f32);
        let mapa = |b: &Caja| -> Caja {
            let r = match k {
                1 => [b[1], h0 - b[2], b[3], h0 - b[0]],
                2 => [w0 - b[2], h0 - b[3], w0 - b[0], h0 - b[1]],
                3 => [w0 - b[3], b[0], w0 - b[1], b[2]],
                _ => *b,
            };
            [
                (r[0] * sx).clamp(0.0, aw),
                (r[1] * sy).clamp(0.0, ah),
                (r[2] * sx).clamp(0.0, aw),
                (r[3] * sy).clamp(0.0, ah),
            ]
        };
        for l in &mut self.lineas {
            l.caja = mapa(&l.caja);
            for w in &mut l.palabras {
                w.caja = mapa(&w.caja);
            }
        }
        for b in &mut self.bloques {
            b.caja = mapa(&b.caja);
        }
        let mut angulo = (self.inclinacion - 90.0 * k as f64) as f32;
        while angulo <= -180.0 {
            angulo += 360.0;
        }
        while angulo > 180.0 {
            angulo -= 360.0;
        }
        angulo
    }
}

/// Lo que dice `meta.json` y hace falta.
#[derive(Debug, Clone)]
pub struct Meta {
    pub vocab: Vocab,
    pub lado_largo: usize,
    pub alto_rec: usize,
    pub ventana: usize,
    pub alfa: f64,
    pub beta: f64,
    pub puerta: f64,
}

impl Meta {
    pub fn de_json(texto: &str) -> Result<Meta, String> {
        let v: serde_json::Value =
            serde_json::from_str(texto).map_err(|e| format!("meta.json: {e}"))?;
        let vocab = Vocab::de_json(
            v.get("vocab")
                .and_then(|x| x.as_str())
                .ok_or("meta.json sin vocab")?,
        )?;
        let entero = |k: &str| v.get(k).and_then(|x| x.as_u64()).map(|x| x as usize);
        let base = v.get("lm_params").and_then(|p| p.get("base"));
        let real = |k: &str, d: f64| {
            base.and_then(|b| b.get(k))
                .and_then(|x| x.as_f64())
                .unwrap_or(d)
        };
        Ok(Meta {
            vocab,
            lado_largo: entero("det_long_side").ok_or("meta.json sin det_long_side")?,
            alto_rec: entero("rec_height").ok_or("meta.json sin rec_height")?,
            ventana: entero("rec_window").unwrap_or(1).max(1),
            alfa: real("alpha", ALFA_LM),
            beta: real("beta", BETA_LM),
            puerta: real("gate", PUERTA_LM),
        })
    }
}

/// La lectura de un renglon.
#[derive(Debug, Clone, Default)]
struct Lectura {
    conf: f32,
    fichas: Vec<Ficha>,
    texto_dudoso: String,
    fichas_dudosas: Vec<Ficha>,
}

/// Una salida CTC: clase, x en la pagina y probabilidad.
#[derive(Debug, Clone, Copy)]
struct Ficha {
    clase: u32,
    x: f32,
    p: f32,
}

struct Renglon {
    caja: Caja,
    texto: String,
    conf: f32,
    fichas: Vec<Ficha>,
    origen: usize,
}

/// El lector, con lo que no son modelos ONNX: el vocabulario, el LM y la
/// correccion.
pub struct Indra {
    pub meta: Meta,
    pub lm: Option<Lm>,
    pub lexico: Option<Hunspell>,
    pub vocabulario: Vocabulario,
    pub orientacion_min: f32,
    pub suelo_duda: f32,
    /// Preparar la pagina antes de leer (fondo a blanco y enfoque).
    pub preparar: bool,
}

impl Indra {
    pub fn nuevo(meta: Meta, lm: Option<Lm>) -> Indra {
        Indra {
            meta,
            lm,
            lexico: None,
            vocabulario: Vocabulario::default(),
            orientacion_min: ORIENTACION_MIN,
            suelo_duda: SUELO_DUDA,
            preparar: true,
        }
    }

    /// **Lee una pagina**: la prepara, la lee y corrige con el diccionario y
    /// el vocabulario. Cajas en pixeles de `g` girada (ver [`Pagina`]).
    pub fn leer(&mut self, m: &mut dyn Modelos, g: &Gris) -> Result<Pagina, String> {
        let t0 = std::time::Instant::now();
        let preparada = if self.preparar && crate::prep::conviene(g) {
            crate::prep::preparar(g)
        } else {
            g.clone()
        };
        let ms_prep = t0.elapsed().as_millis() as u64;
        let mut p = self.ejecutar(m, &preparada)?;
        p.ms.insert("prep_ms", ms_prep);
        let t = std::time::Instant::now();
        let c = Corrector {
            lexico: self.lexico.as_ref(),
            vocabulario: &self.vocabulario,
        };
        let d = self.lm.as_mut().map(|lm| Desempate {
            lm,
            vocab: &self.meta.vocab,
        });
        c.corregir(&mut p, d);
        p.ms.insert("correccion_ms", t.elapsed().as_millis() as u64);
        Ok(p)
    }

    /// La pagina sin preparar ni corregir (`run` del escaner).
    pub fn ejecutar(&mut self, m: &mut dyn Modelos, g: &Gris) -> Result<Pagina, String> {
        let mut ms = HashMap::new();
        let t0 = std::time::Instant::now();
        let previa = imagen::previa(g, imagen::LADO_PREVIA);
        let mut a = imagen::estimar_inclinacion(&previa, 800);
        let k = {
            let p = if a.abs() >= INCLINACION_MIN {
                imagen::girar_pagina(&previa, a)
            } else {
                previa.clone()
            };
            self.orientacion(m, &p)?
        };
        let mut pag = if k != 0 { rot90(g, -k) } else { g.clone() };
        if k == 1 || k == 3 {
            a = imagen::estimar_inclinacion(&rot90(&previa, -k), 800);
        }
        let incl = if a.abs() >= INCLINACION_MIN { a } else { 0.0 };
        if incl != 0.0 {
            pag = imagen::girar_pagina(&pag, incl);
        }
        ms.insert("orientacion_ms", t0.elapsed().as_millis() as u64);
        let t = std::time::Instant::now();
        let cajas = self.detectar(m, &pag)?;
        ms.insert("deteccion_ms", t.elapsed().as_millis() as u64);
        let t = std::time::Instant::now();
        let lecturas = self.reconocer(m, &pag, &cajas, 64)?;
        ms.insert("reconocimiento_ms", t.elapsed().as_millis() as u64);
        let t = std::time::Instant::now();
        let mut p = self.ensamblar(m, pag.ancho, pag.alto, &cajas, &lecturas)?;
        ms.insert("orden_ms", t.elapsed().as_millis() as u64);
        if incl != 0.0 {
            let (w, h) = (pag.ancho, pag.alto);
            for l in &mut p.lineas {
                l.caja = imagen::desgirar_caja(&l.caja, incl, w, h);
                for wd in &mut l.palabras {
                    wd.caja = imagen::desgirar_caja(&wd.caja, incl, w, h);
                }
            }
            for b in &mut p.bloques {
                b.caja = imagen::desgirar_caja(&b.caja, incl, w, h);
            }
        }
        p.giro = 90 * k;
        p.inclinacion = incl;
        ms.insert("total_ms", t0.elapsed().as_millis() as u64);
        p.ms = ms;
        Ok(p)
    }

    /// Cuartos de vuelta (antihorario) del contenido; 0 si ninguno supera
    /// el umbral o la pagina va en columnas verticales.
    fn orientacion(&mut self, m: &mut dyn Modelos, g: &Gris) -> Result<i32, String> {
        let x = imagen::entrada_orientacion(g, imagen::LADO_ORIENTACION);
        let Some(p) = m.orientar(x, imagen::LADO_ORIENTACION)? else {
            return Ok(0);
        };
        let mut k = 0;
        for i in 0..p.len() {
            if p[i] > p[k] {
                k = i;
            }
        }
        Ok(
            if p.get(k).is_some_and(|&v| v > self.orientacion_min) && k < 4 {
                k as i32
            } else {
                0
            },
        )
    }

    fn detectar_a(
        &self,
        m: &mut dyn Modelos,
        g: &Gris,
        lado: usize,
    ) -> Result<(Vec<Caja>, f64), String> {
        let e = imagen::entrada_detector(g, lado);
        let (s, ho, wo) = m.detectar(e.datos, e.alto, e.ancho)?;
        Ok((deteccion::decodificar(&s, wo, ho, 0.5, 2), e.escala))
    }

    /// Renglones en pixeles de la pagina, con una segunda pasada a mas
    /// resolucion si la letra es diminuta.
    fn detectar(&self, m: &mut dyn Modelos, g: &Gris) -> Result<Vec<Caja>, String> {
        let lado = self.meta.lado_largo;
        let (mut cajas, mut s) = self.detectar_a(m, g, lado)?;
        if cajas.len() >= 8 {
            let alturas: Vec<f32> = cajas.iter().map(|b| b[3] - b[1]).collect();
            let med = orden::mediana(&alturas);
            if med < ALTO_MIN_ADAPTATIVO {
                let ls = ((LADO_MAX_ADAPTATIVO as f64)
                    .min(lado as f64 * ALTO_OBJETIVO_ADAPTATIVO / (med as f64).max(1.0))
                    as usize)
                    / 32
                    * 32;
                if ls > lado {
                    (cajas, s) = self.detectar_a(m, g, ls)?;
                }
            }
        }
        let (w, h) = (g.ancho as f32, g.alto as f32);
        Ok(cajas
            .into_iter()
            .map(|b| {
                [
                    (b[0] as f64 / s).clamp(0.0, w as f64) as f32,
                    (b[1] as f64 / s).clamp(0.0, h as f64) as f32,
                    (b[2] as f64 / s).clamp(0.0, w as f64) as f32,
                    (b[3] as f64 / s).clamp(0.0, h as f64) as f32,
                ]
            })
            .collect())
    }

    fn reconocer(
        &mut self,
        m: &mut dyn Modelos,
        g: &Gris,
        cajas: &[Caja],
        lote: usize,
    ) -> Result<Vec<Lectura>, String> {
        let n = cajas.len();
        let mut out = vec![Lectura::default(); n];
        if n == 0 {
            return Ok(out);
        }
        let alto = self.meta.alto_rec;
        let ventana = self.meta.ventana;
        let oscuras: Vec<Option<bool>> = cajas
            .iter()
            .map(|b| (ventana > 1).then(|| imagen::linea_oscura(g, b)))
            .collect();
        let recortes: Vec<imagen::Recorte> = (0..n)
            .map(|i| imagen::recortar_linea(g, &cajas[i], alto, 1, oscuras[i], 4096))
            .collect();
        let altos: Vec<imagen::Recorte> = if ventana > 1 {
            (0..n)
                .map(|i| imagen::recortar_linea(g, &cajas[i], alto, ventana, oscuras[i], 4096))
                .collect()
        } else {
            recortes.clone()
        };
        let con_lm = self.lm.is_some();
        let suelo = CONFIANZA_MIN.min(self.suelo_duda);
        let anchos: Vec<usize> = recortes.iter().map(|r| r.ancho).collect();
        for idx in imagen::lotes_por_ancho(&anchos, lote) {
            let b = idx.len();
            let hh = altos[idx[0]].alto;
            let wm = idx
                .iter()
                .map(|&j| altos[j].ancho)
                .max()
                .unwrap_or(8)
                .div_ceil(imagen::MULTIPLO_ANCHO)
                * imagen::MULTIPLO_ANCHO;
            let mut arr = vec![0f32; b * hh * wm];
            let mut w_util = Vec::with_capacity(b);
            for (k, &j) in idx.iter().enumerate() {
                let c = &altos[j];
                for y in 0..hh {
                    arr[(k * hh + y) * wm..(k * hh + y) * wm + c.ancho]
                        .copy_from_slice(&c.datos[y * c.ancho..(y + 1) * c.ancho]);
                }
                w_util.push(c.ancho as i64);
            }
            let r = m.reconocer(arr, b, hh, wm, w_util, con_lm)?;
            let t_pasos = r.pasos;
            let paso_x = wm as f64 / t_pasos.max(1) as f64;
            for (k, &j) in idx.iter().enumerate() {
                let largo =
                    (r.longitudes.get(k).copied().unwrap_or(0).max(0) as usize).min(t_pasos);
                let mut guardar = Vec::new();
                let mut suma = 0.0f64;
                let mut previo = -1i64;
                for s in 0..largo {
                    let v = r.ids[k * t_pasos + s];
                    if v != 0 && (s == 0 || v != previo) {
                        guardar.push(s);
                        suma += r.conf[k * t_pasos + s] as f64;
                    }
                    previo = v;
                }
                let conf = if guardar.is_empty() {
                    0.0
                } else {
                    (suma / guardar.len() as f64) as f32
                };
                let mut clases: Vec<u32> = guardar
                    .iter()
                    .map(|&s| r.ids[k * t_pasos + s] as u32)
                    .collect();
                let mut pasos: Vec<u32> = guardar.iter().map(|&s| s as u32).collect();
                let mut probs: Vec<f32> =
                    guardar.iter().map(|&s| r.conf[k * t_pasos + s]).collect();
                if let (Some(lm), Some((blanco, topv, topi, kt))) =
                    (self.lm.as_mut(), r.lm.as_ref())
                    && conf >= suelo
                    && (conf as f64) < self.meta.puerta
                {
                    let kt = *kt;
                    let base = k * t_pasos;
                    let e = SalidaDeLinea {
                        blanco: &blanco[base..base + largo],
                        topv: &topv[base * kt..(base + largo) * kt],
                        topi: &topi[base * kt..(base + largo) * kt],
                        k: kt,
                        pasos: largo,
                    };
                    let c = Corrector {
                        lexico: self.lexico.as_ref(),
                        vocabulario: &self.vocabulario,
                    };
                    let sesgo = c.sesgo();
                    let (bi, bs) = lm.buscar_en_haz(
                        &e,
                        self.meta.alfa,
                        self.meta.beta,
                        HAZ_LM,
                        0.999,
                        SUELO_CANDIDATO_LM,
                        Some((&sesgo, &self.meta.vocab)),
                    );
                    probs = bi
                        .iter()
                        .zip(&bs)
                        .map(|(&c, &s0)| {
                            let s0 = s0 as usize;
                            (0..kt)
                                .find(|&jj| e.topi[s0 * kt + jj] as u32 == c)
                                .map_or(0.01, |jj| e.topv[s0 * kt + jj].exp())
                        })
                        .collect();
                    clases = bi;
                    pasos = bs;
                }
                let bx = cajas[j];
                let h = ((bx[3] - bx[1]) as f64).max(1.0);
                let ix0 = (bx[0] as f64 - imagen::MARGEN_X * h).floor();
                let cw = (bx[2] as f64 + imagen::MARGEN_X * h).ceil() - ix0;
                let f = cw / recortes[j].ancho as f64;
                let fichas: Vec<Ficha> = clases
                    .iter()
                    .zip(&pasos)
                    .zip(&probs)
                    .map(|((&c, &s), &p)| Ficha {
                        clase: c,
                        x: (ix0 + (s as f64 + 0.5) * paso_x * f) as f32,
                        p,
                    })
                    .collect();
                let texto = self.meta.vocab.decodificar(&clases);
                out[j] = if conf >= CONFIANZA_MIN {
                    Lectura {
                        conf,
                        fichas,
                        ..Default::default()
                    }
                } else if conf >= suelo {
                    Lectura {
                        conf,
                        texto_dudoso: texto,
                        fichas_dudosas: fichas,
                        ..Default::default()
                    }
                } else {
                    Lectura {
                        conf,
                        ..Default::default()
                    }
                };
            }
        }
        Ok(out)
    }

    fn es_espacio(&self, f: &Ficha) -> bool {
        Some(f.clase) == self.meta.vocab.espacio
    }

    fn texto_de(&self, f: &[Ficha]) -> String {
        let c: Vec<u32> = f.iter().map(|x| x.clase).collect();
        self.meta.vocab.decodificar(&c)
    }

    fn cortes_de_calle(&self, cajas: &[Caja], lect: &[Lectura]) -> Vec<Vec<f32>> {
        cajas
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let h = (b[3] - b[1]).max(1.0);
                let xs: Vec<f32> = lect[i]
                    .fichas
                    .iter()
                    .filter(|f| !self.es_espacio(f))
                    .map(|f| f.x)
                    .collect();
                let mut mios = Vec::new();
                for q in 1..xs.len() {
                    let (a, c) = (xs[q - 1], xs[q]);
                    if c - a <= HUECO_CALLE * h {
                        continue;
                    }
                    let (gl, gr) = (a + 0.35 * h, c - 0.35 * h);
                    let cerca = cajas
                        .iter()
                        .enumerate()
                        .filter(|&(o, ob)| {
                            o != i && ((ob[2] - gl).abs() < 0.5 * h || (ob[0] - gr).abs() < 0.5 * h)
                        })
                        .count();
                    if cerca >= BORDES_CALLE {
                        mios.push((a + c) / 2.0);
                    }
                }
                mios
            })
            .collect()
    }

    fn partir(
        &self,
        caja: &Caja,
        fichas: &[Ficha],
        hueco: f32,
        cortes: &[f32],
    ) -> Vec<(Caja, String, Vec<Ficha>)> {
        let h = (caja[3] - caja[1]).max(1.0);
        let mut trozos: Vec<Vec<Ficha>> = vec![Vec::new()];
        let mut previo: Option<f32> = None;
        let mut cortes = cortes.to_vec();
        cortes.sort_by(f32::total_cmp);
        for f in fichas {
            if !self.es_espacio(f) {
                if let Some(p) = previo
                    && (f.x - p > hueco * h || cortes.iter().any(|&c| p < c && c < f.x))
                {
                    trozos.push(Vec::new());
                }
                previo = Some(f.x);
            }
            if let Some(t) = trozos.last_mut() {
                t.push(*f);
            }
        }
        if trozos.len() == 1 {
            return vec![(*caja, self.texto_de(fichas), fichas.to_vec())];
        }
        let mut o = Vec::new();
        for t in trozos {
            let texto = self.texto_de(&t).trim().to_string();
            let xs: Vec<f32> = t
                .iter()
                .filter(|f| !self.es_espacio(f))
                .map(|f| f.x)
                .collect();
            if !texto.is_empty() && !xs.is_empty() {
                let mn = xs.iter().copied().fold(f32::MAX, f32::min);
                let mx = xs.iter().copied().fold(f32::MIN, f32::max);
                o.push((
                    [
                        caja[0].max(mn - 0.35 * h),
                        caja[1],
                        caja[2].min(mx + 0.35 * h),
                        caja[3],
                    ],
                    texto,
                    t,
                ));
            }
        }
        o
    }

    /// Fuera lineas de puro relleno (puntos, rayas); viñetas y numeros
    /// sueltos unidos a su elemento (`tidy_lines`).
    fn ordenar_lineas(&self, mut l: Vec<Renglon>) -> Vec<Renglon> {
        let n = l.len();
        let mut queda = vec![true; n];
        for (i, r) in l.iter().enumerate() {
            let nucleo: String = r.texto.chars().filter(|&c| c != ' ').collect();
            if !nucleo.is_empty() && nucleo.chars().all(|c| RELLENO.contains(c)) {
                let w = r.caja[2] - r.caja[0];
                let h = r.caja[3] - r.caja[1];
                if nucleo.chars().count() >= 3
                    || w >= 3.0 * h
                    || !nucleo.chars().any(|c| GUIONES.contains(c))
                {
                    queda[i] = false;
                }
            }
        }
        let hs: Vec<f32> = l.iter().map(|r| r.caja[3] - r.caja[1]).collect();
        for i in 0..n {
            let t = l[i].texto.trim().to_string();
            let unico =
                t.chars().count() == 1 && t.chars().next().is_some_and(|c| VINETAS.contains(c));
            if !queda[i] || !(unico || es_enumerador(&t)) {
                continue;
            }
            let bi = l[i].caja;
            let hi = hs[i];
            let mut mejor: Option<usize> = None;
            let mut hueco_mejor = 0f32;
            for j in 0..n {
                if j == i || !queda[j] || l[j].texto.trim().is_empty() {
                    continue;
                }
                let bj = l[j].caja;
                let hj = hs[j];
                let solape = bi[3].min(bj[3]) - bi[1].max(bj[1]);
                if solape < 0.5 * hi.min(hj) {
                    continue;
                }
                let hueco = bj[0] - bi[2];
                if hueco >= -0.3 * hi
                    && hueco <= HUECO_MARCADOR * hi.max(hj)
                    && (mejor.is_none() || hueco < hueco_mejor)
                {
                    mejor = Some(j);
                    hueco_mejor = hueco;
                }
            }
            let Some(j) = mejor else { continue };
            if let Some(sp) = self.meta.vocab.espacio
                && !l[i].fichas.is_empty()
                && !l[j].fichas.is_empty()
            {
                let x = (bi[2] + l[j].caja[0]) / 2.0;
                let mut f = l[i].fichas.clone();
                f.push(Ficha {
                    clase: sp,
                    x,
                    p: 1.0,
                });
                f.extend(l[j].fichas.iter().copied());
                l[j].fichas = f;
            }
            l[j].texto = format!("{t} {}", l[j].texto.trim());
            let bj = l[j].caja;
            l[j].caja = [
                bi[0].min(bj[0]),
                bi[1].min(bj[1]),
                bi[2].max(bj[2]),
                bi[3].max(bj[3]),
            ];
            queda[i] = false;
        }
        l.into_iter()
            .zip(queda)
            .filter(|(_, q)| *q)
            .map(|(r, _)| r)
            .collect()
    }

    /// Palabras de una linea: tramos entre espacios, con la caja desde las
    /// posiciones de los picos CTC.
    fn palabras(&self, caja: &Caja, fichas: &[Ficha]) -> Vec<Palabra> {
        let h = (caja[3] - caja[1]).max(1.0);
        let mut o = Vec::new();
        let mut cur: Vec<Ficha> = Vec::new();
        let vaciar = |cur: &mut Vec<Ficha>, o: &mut Vec<Palabra>| {
            if cur.is_empty() {
                return;
            }
            let texto = self.texto_de(cur).trim().to_string();
            if !texto.is_empty() {
                let mn = cur.iter().map(|f| f.x).fold(f32::MAX, f32::min);
                let mx = cur.iter().map(|f| f.x).fold(f32::MIN, f32::max);
                let x0 = caja[0].max(mn - 0.35 * h);
                let x1 = caja[2].min(mx + 0.35 * h);
                let ps: f32 = cur.iter().map(|f| f.p).sum();
                let probs =
                    (texto.chars().count() == cur.len()).then(|| cur.iter().map(|f| f.p).collect());
                o.push(Palabra {
                    texto,
                    caja: [x0, caja[1], x1.max(x0 + 1.0), caja[3]],
                    confianza: ps / cur.len() as f32,
                    probs,
                });
            }
            cur.clear();
        };
        for f in fichas {
            if self.es_espacio(f) {
                vaciar(&mut cur, &mut o);
            } else {
                cur.push(*f);
            }
        }
        vaciar(&mut cur, &mut o);
        o
    }

    fn ensamblar(
        &mut self,
        m: &mut dyn Modelos,
        w: usize,
        h: usize,
        cajas: &[Caja],
        lect: &[Lectura],
    ) -> Result<Pagina, String> {
        let cortes = self.cortes_de_calle(cajas, lect);
        let mut partidas = Vec::new();
        for (i, b) in cajas.iter().enumerate() {
            for (caja, texto, fichas) in self.partir(b, &lect[i].fichas, HUECO_CORTE, &cortes[i]) {
                partidas.push(Renglon {
                    caja,
                    texto,
                    conf: lect[i].conf,
                    fichas,
                    origen: i,
                });
            }
        }
        let lineas = self.ordenar_lineas(partidas);
        let bx: Vec<Caja> = lineas.iter().map(|l| l.caja).collect();
        let textos: Vec<String> = lineas.iter().map(|l| l.texto.clone()).collect();
        let n = lineas.len();
        let mut cls: Option<Vec<Vec<f32>>> = None;
        let mut mismo: Option<Vec<Vec<f32>>> = None;
        let salida = if n >= 2 {
            m.ordenar(orden::rasgos(&bx, w as f32, h as f32, &textos), n)?
        } else {
            None
        };
        let ord: Vec<usize> = match salida {
            None => orden::orden_xy(&bx),
            Some(s) => {
                let cuadrada = |v: &[f32], m2: usize| -> Vec<Vec<f32>> {
                    (0..n)
                        .map(|i| (0..n).map(|j| v[i * m2 + j]).collect())
                        .collect()
                };
                let m2 = if s.n2 == 0 { n } else { s.n2 };
                let a = cuadrada(&s.a, m2);
                match (s.cls, s.mismo) {
                    (Some(c), Some(sm)) => {
                        let nc = c.len() / n;
                        let c: Vec<Vec<f32>> =
                            (0..n).map(|i| c[i * nc..(i + 1) * nc].to_vec()).collect();
                        let sm = cuadrada(&sm, m2);
                        let o = orden::decodificar_orden_por_bloques(&a, &sm, &bx, Some(&c));
                        cls = Some(c);
                        mismo = Some(sm);
                        o
                    }
                    _ => {
                        let a64: Vec<Vec<f64>> = a
                            .iter()
                            .map(|f| f.iter().map(|&v| v as f64).collect())
                            .collect();
                        orden::decodificar_orden(&a64, 4)
                    }
                }
            }
        };
        let mut pos = vec![0usize; n];
        for (k, &i) in ord.iter().enumerate() {
            pos[i] = k;
        }
        let alturas: Vec<f32> = lineas.iter().map(|l| l.caja[3] - l.caja[1]).collect();
        let med = if n == 0 {
            1.0
        } else {
            orden::mediana(&alturas)
        };
        let r1 = |c: &Caja| c.map(|v| (v * 10.0).round() / 10.0);
        let mut salida_lineas = Vec::with_capacity(n);
        for &i in &ord {
            let l = &lineas[i];
            let r = &lect[l.origen];
            if l.texto.trim().is_empty() && !r.fichas_dudosas.is_empty() {
                let t = r.texto_dudoso.trim().to_string();
                let bh = l.caja[3] - l.caja[1];
                if dudas::conservar(&t, l.conf, bh / med.max(1.0), self.suelo_duda) {
                    salida_lineas.push(Linea {
                        caja: r1(&l.caja),
                        texto: t,
                        confianza: l.conf,
                        palabras: self.palabras(&l.caja, &r.fichas_dudosas),
                        dudosa: true,
                    });
                    continue;
                }
                salida_lineas.push(Linea {
                    caja: r1(&l.caja),
                    texto: String::new(),
                    confianza: l.conf,
                    palabras: Vec::new(),
                    dudosa: false,
                });
                continue;
            }
            salida_lineas.push(Linea {
                caja: r1(&l.caja),
                texto: l.texto.trim().to_string(),
                confianza: l.conf,
                palabras: self.palabras(&l.caja, &l.fichas),
                dudosa: false,
            });
        }
        let bloques = match (cls, mismo) {
            (Some(c), Some(sm)) => orden::bloques_de_pagina(&bx, &ord, &c, &sm)
                .into_iter()
                .map(|b| Bloque {
                    lineas: b.lineas.iter().map(|&i| pos[i]).collect(),
                    ..b
                })
                .collect(),
            _ if n > 0 => orden::bloques_xy(&bx, 1.0)
                .into_iter()
                .map(|blk| {
                    let mut lineas: Vec<usize> = blk.iter().map(|&i| pos[i]).collect();
                    lineas.sort_unstable();
                    Bloque {
                        etiqueta: "Text",
                        caja: orden::caja_de(&bx, &blk),
                        lineas,
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(Pagina {
            lineas: salida_lineas,
            bloques,
            giro: 0,
            inclinacion: 0.0,
            ancho: w,
            alto: h,
            ms: HashMap::new(),
        })
    }
}

/// «1.», «a)», «(iv)»…: un numero de lista suelto.
fn es_enumerador(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    let cuerpo_ok = |s: &[char], romanos: bool| {
        (!s.is_empty() && s.len() <= 3 && s.iter().all(|c| c.is_ascii_digit()))
            || (s.len() == 1 && s[0].is_ascii_alphabetic())
            || (romanos
                && !s.is_empty()
                && s.len() <= 5
                && s.iter().all(|c| "ivxlcIVXLC".contains(*c)))
    };
    if c.len() >= 2
        && (c[c.len() - 1] == '.' || c[c.len() - 1] == ')')
        && cuerpo_ok(&c[..c.len() - 1], false)
    {
        return true;
    }
    c.len() >= 3 && c[0] == '(' && c[c.len() - 1] == ')' && cuerpo_ok(&c[1..c.len() - 1], true)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Modelos de mentira: un detector que ve los renglones que se le
    /// digan y un reconocedor que «lee» siempre lo mismo.
    struct Falsos {
        /// Cajas de nucleo en la salida del detector `(x0, y0, x1, y1)`.
        nucleos: Vec<(usize, usize, usize, usize)>,
        /// Lo que lee cada linea, en clases, y su confianza.
        lee: Vec<(Vec<u32>, f32)>,
        orden: Option<SalidaOrden>,
    }

    impl Modelos for Falsos {
        fn detectar(
            &mut self,
            _x: Vec<f32>,
            alto: usize,
            ancho: usize,
        ) -> Result<(Vec<f32>, usize, usize), String> {
            let (h, w) = (alto / 2, ancho / 2);
            let mut s = vec![-10.0; 3 * w * h];
            for &(x0, y0, x1, y1) in &self.nucleos {
                for y in y0..y1 {
                    for x in x0..x1 {
                        s[y * w + x] = 10.0;
                        s[w * h + y * w + x] = 6.0;
                        s[2 * w * h + y * w + x] = 6.0;
                    }
                }
            }
            Ok((s, h, w))
        }
        fn reconocer(
            &mut self,
            _x: Vec<f32>,
            lote: usize,
            _alto: usize,
            ancho: usize,
            _anchos: Vec<i64>,
            _con_lm: bool,
        ) -> Result<SalidaRec, String> {
            let pasos = ancho / 4;
            let mut r = SalidaRec {
                ids: vec![0; lote * pasos],
                conf: vec![0.0; lote * pasos],
                longitudes: vec![pasos as i64; lote],
                pasos,
                lm: None,
            };
            for k in 0..lote {
                let (clases, c) = &self.lee[k % self.lee.len()];
                for (i, &cl) in clases.iter().enumerate() {
                    // una clase cada dos pasos, con blanco en medio
                    let s = 2 * i;
                    if s < pasos {
                        r.ids[k * pasos + s] = cl as i64;
                        r.conf[k * pasos + s] = *c;
                    }
                }
            }
            Ok(r)
        }
        fn orientar(&mut self, _x: Vec<f32>, _lado: usize) -> Result<Option<Vec<f32>>, String> {
            Ok(Some(vec![0.99, 0.0, 0.0, 0.01, 0.0]))
        }
        fn ordenar(&mut self, _r: Vec<f32>, _n: usize) -> Result<Option<SalidaOrden>, String> {
            Ok(self.orden.clone())
        }
    }

    fn meta() -> Meta {
        Meta::de_json(
            r#"{"vocab": "{\"blank\": 0, \"chars\": [\" \", \"H\", \"o\", \"l\", \"a\", \"-\"]}",
                "det_long_side": 256, "rec_height": 32, "rec_window": 1}"#,
        )
        .unwrap()
    }

    fn pagina_blanca() -> Gris {
        Gris::lisa(256, 128, 255)
    }

    #[test]
    fn meta_json_con_y_sin_parametros_del_lm() {
        let m = meta();
        assert_eq!((m.lado_largo, m.alto_rec, m.ventana), (256, 32, 1));
        assert_eq!(m.alfa, ALFA_LM);
        let m = Meta::de_json(
            r#"{"vocab": "{\"chars\": []}", "det_long_side": 1024, "rec_height": 32, "rec_window": 3,
                "lm_params": {"base": {"alpha": 0.7, "gate": 0.8}}}"#,
        )
        .unwrap();
        assert_eq!(
            (m.ventana, m.alfa, m.beta, m.puerta),
            (3, 0.7, BETA_LM, 0.8)
        );
        assert!(Meta::de_json("{}").is_err());
    }

    #[test]
    fn dos_renglones_salen_en_orden_con_palabras_y_bloques() {
        let mut f = Falsos {
            nucleos: vec![(10, 40, 60, 42), (10, 10, 60, 12)],
            lee: vec![(vec![2, 3, 4, 5, 1, 2, 3, 4, 5], 0.95)],
            orden: None,
        };
        let mut indra = Indra::nuevo(meta(), None);
        indra.preparar = false;
        let p = indra.ejecutar(&mut f, &pagina_blanca()).unwrap();
        assert_eq!(p.lineas.len(), 2);
        // El de arriba primero (XY-cut sin modelo de orden).
        assert!(p.lineas[0].caja[1] < p.lineas[1].caja[1]);
        assert_eq!(p.lineas[0].texto, "Hola Hola");
        assert_eq!(p.lineas[0].palabras.len(), 2);
        assert!(p.lineas[0].palabras[0].caja[2] <= p.lineas[0].palabras[1].caja[0] + 1.0);
        assert_eq!(p.giro, 0);
        assert_eq!(p.texto(), "Hola Hola\nHola Hola");
        assert!(!p.bloques.is_empty());
        assert!(p.ms.contains_key("total_ms"));
    }

    #[test]
    fn una_linea_de_baja_confianza_queda_dudosa_o_vacia() {
        let mut f = Falsos {
            nucleos: vec![(10, 10, 60, 12)],
            lee: vec![(vec![2, 3, 4, 5, 1, 2, 3, 4, 5], 0.42)],
            orden: None,
        };
        let mut indra = Indra::nuevo(meta(), None);
        indra.preparar = false;
        let p = indra.ejecutar(&mut f, &pagina_blanca()).unwrap();
        assert_eq!(p.lineas.len(), 1);
        assert!(p.lineas[0].dudosa);
        assert_eq!(p.lineas[0].texto, "Hola Hola");
        assert_eq!(p.texto(), "", "las dudosas no van al texto plano");
        // Por debajo del suelo: caja sin texto.
        f.lee = vec![(vec![2, 3], 0.1)];
        let p = indra.ejecutar(&mut f, &pagina_blanca()).unwrap();
        assert!(!p.lineas[0].dudosa && p.lineas[0].texto.is_empty());
    }

    #[test]
    fn las_rayas_de_relleno_se_quitan() {
        let mut f = Falsos {
            nucleos: vec![(10, 10, 60, 12)],
            lee: vec![(vec![6, 6, 6, 6], 0.95)],
            orden: None,
        };
        let mut indra = Indra::nuevo(meta(), None);
        indra.preparar = false;
        let p = indra.ejecutar(&mut f, &pagina_blanca()).unwrap();
        assert!(p.lineas.is_empty());
    }

    #[test]
    fn con_el_modelo_de_orden_manda_su_orden_y_sus_bloques() {
        // Dos lineas; el modelo dice que la de abajo va primero y que son
        // bloques distintos (cabecera y texto).
        let a = vec![0.0, -4.0, 4.0, 0.0];
        let mismo = vec![0.0, -3.0, -3.0, 0.0];
        let mut cls = vec![0.0f32; 20];
        cls[1] = 5.0; // linea 0: Section-Header
        cls[10] = 5.0; // linea 1: Text
        let mut f = Falsos {
            nucleos: vec![(10, 10, 60, 12), (10, 40, 60, 42)],
            lee: vec![(vec![2, 3, 4, 5], 0.95)],
            orden: Some(SalidaOrden {
                a,
                n2: 2,
                cls: Some(cls),
                mismo: Some(mismo),
            }),
        };
        let mut indra = Indra::nuevo(meta(), None);
        indra.preparar = false;
        let p = indra.ejecutar(&mut f, &pagina_blanca()).unwrap();
        assert_eq!(p.lineas.len(), 2);
        assert!(
            p.lineas[0].caja[1] > p.lineas[1].caja[1],
            "la de abajo primero"
        );
        assert_eq!(p.bloques.len(), 2);
        assert_eq!(p.bloques[0].etiqueta, "Text");
        assert_eq!(p.bloques[1].etiqueta, "Section-Header");
        assert_eq!(p.bloques[0].lineas, [0]);
    }

    #[test]
    fn cajas_de_vuelta_a_la_imagen_original() {
        // IndraUnitTest.cajasDeVueltaALaImagenOriginal: imagen 100x200 que
        // Indra giro 90 (pagina girada 200x100).
        let pal = Palabra {
            texto: "x".into(),
            caja: [175.0, 5.0, 185.0, 15.0],
            confianza: 0.9,
            probs: None,
        };
        let l = Linea {
            caja: [175.0, 5.0, 185.0, 15.0],
            texto: "x".into(),
            confianza: 0.9,
            palabras: vec![pal],
            dudosa: false,
        };
        let mut p = Pagina {
            lineas: vec![l.clone()],
            bloques: vec![],
            giro: 90,
            inclinacion: 0.0,
            ancho: 200,
            alto: 100,
            ms: HashMap::new(),
        };
        let ang = p.a_la_imagen(100, 200, 1.0, 1.0);
        assert_eq!(p.lineas[0].caja, [5.0, 15.0, 15.0, 25.0]);
        assert!((ang + 90.0).abs() < 1e-4);
        let mut p0 = Pagina {
            lineas: vec![l],
            bloques: vec![],
            giro: 0,
            inclinacion: 1.5,
            ancho: 200,
            alto: 100,
            ms: HashMap::new(),
        };
        let ang = p0.a_la_imagen(300, 150, 1.5, 1.5);
        assert!((p0.lineas[0].caja[0] - 262.5).abs() < 1e-3);
        assert!((ang - 1.5).abs() < 1e-4);
        assert!(p0.lineas[0].caja[2] <= 300.0);
    }

    #[test]
    fn enumeradores() {
        for t in ["1.", "12)", "a)", "(iv)", "(3)"] {
            assert!(es_enumerador(t), "{t}");
        }
        for t in ["1", "hola.", "(abc)", "1234."] {
            assert!(!es_enumerador(t), "{t}");
        }
    }
}
