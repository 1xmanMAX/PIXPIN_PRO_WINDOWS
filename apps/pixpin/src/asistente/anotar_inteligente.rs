//! **`anotar_inteligente`, del lado de la app** (15-oct-2026): leer la
//! pagina CON donde cae cada palabra, pedirle al modelo las anotaciones y
//! pintarlas en la capa del documento segun llegan.
//!
//! El motor es uno (`pixpin_ia::anotacion`); lo que cambia de un formato a
//! otro son dos cosas, y solo ellas estan aqui:
//!
//! - **Leer** ([`leer`]): la capa de texto del PDF (y el OCR si la hoja no
//!   tiene texto: un escaneado), la maquetacion de un Word o un PowerPoint
//!   (la misma del lector), el OCR con cajas de una foto, los textos de un
//!   plano o las celdas de un Excel; todo pasado a las unidades de su capa.
//! - **Escribir** ([`Destino`]): la MISMA capa que leen y sincronizan los
//!   visores (la hoja del lector de PDF y Word por `DondeVa`, el
//!   `.pixpin2d` de la foto, la `anot-<uid>` del plano, la capa de la hoja
//!   de calculo junto al libro), con las figuras del motor comun.
//!
//! **Latencia**: la respuesta llega en vivo; cada anotacion se ancla,
//! coloca y escribe en cuanto su objeto se cierra (milisegundos), y se avisa
//! a los visores para que la relean ya (`lector_tinta::turnos`). El visor se
//! abre en esa pagina con la primera.
//!
//! **Deshacer**: todo lo del turno lleva el grupo `asistente-anotado:<turno>`
//! y se apunta en `lector_tinta::turnos`: al releer la capa, un Ctrl+Z lo
//! quita entero (en un PDF, de todas las hojas a la vez).
//!
//! **Para la revision visual** (la de `ia-ojos-lienzo`): [`Hecho::capas`]
//! dice que ficheros de capa se tocaron y en que pagina, y
//! `pixpin_ia::anotacion::GRUPO_TURNO` + el turno, que figuras son de este
//! turno; `pintar_para_revisar` (en las pruebas, `banco.rs`) pinta la pagina
//! con ellas a PNG por el componente «Exportar».

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use pixpin_ia::anotacion::{self, Anotador, Leida, Pagina, Palabra, Renglon};
use pixpin_ia::motor::{Mensaje, Motor, Peticion, Seguir, Trozo};
use pixpin_motor2d::Elemento;
use pixpin_pdf::texto::PaginaDeTexto;
use serde_json::{Value, json};

use super::documentos_acciones::{Contexto, Sirve, elegir};

#[cfg(test)]
pub(crate) mod banco;
#[cfg(test)]
mod pruebas;

const ANCHO_HOJA: f32 = pixpin_docs::vista::ANCHO_HOJA;
/// Cuantas paginas, como mucho, en un pedido.
pub const MAX_PAGINAS: usize = 5;
/// Cuantas hojas escaneadas, como mucho, pasan por el OCR en un pedido.
const MAX_OCR: usize = 3;
/// El ancho al que se pinta una hoja para el OCR (~200 ppp en A4).
const ANCHO_OCR: u32 = 1700;
/// Una foto mas grande se reduce antes del OCR (no gana nada).
const LADO_OCR: u32 = 2600;
/// Filas y celdas que se leen de una hoja de calculo.
const MAX_FILAS: u32 = 400;
const MAX_CELDAS: usize = 1500;

/// Que se anota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Pdf,
    /// Word o PowerPoint, maquetado como en el lector.
    Office,
    Foto,
    Plano,
    /// Excel.
    Libro,
}

pub fn formato_de(ruta: &Path) -> Option<Formato> {
    let nombre = pixpin_docs::nombre(ruta);
    let e = pixpin_docs::extension(&nombre);
    Some(if e == "pdf" {
        Formato::Pdf
    } else if crate::lector_pdf::se_abre_office(&nombre) {
        Formato::Office
    } else if super::ocr::es_imagen(ruta) {
        Formato::Foto
    } else if super::documentos_acciones::es_plano(&nombre) {
        Formato::Plano
    } else if matches!(e.as_str(), "xlsx" | "xlsm" | "xls") {
        Formato::Libro
    } else {
        return None;
    })
}

/// Donde van las figuras de cada pagina.
pub enum Destino {
    /// La hoja `i` de un PDF, Word o PowerPoint (`DondeVa`), con su alto.
    Hojas { altos: HashMap<usize, f32> },
    /// El dibujo de la foto (`<foto>.pixpin2d`, la foto en 0,0 a sus pixeles).
    Foto { dibujo: PathBuf },
    /// La capa del plano (ya en sus unidades).
    Plano { rutas: crate::plano_cad::anotar::Rutas, modelo: Box<pixpin_cad::modelo::Modelo> },
    /// La capa de la hoja `hoja` del libro (junto a el).
    Libro { hoja: usize },
}

/// El documento leido para anotar.
pub struct Leido {
    pub formato: Formato,
    pub paginas: Vec<Pagina>,
    pub destino: Destino,
    /// Lo que tardo leerlo (con el OCR si hizo falta).
    pub ms_leer: u128,
    /// Cuantas paginas salieron del OCR.
    pub con_ocr: usize,
}

fn texto_de<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn entero(args: &Value, campo: &str) -> Option<usize> {
    args.get(campo).and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok())).map(|n| n as usize)
}

/// Las paginas pedidas (desde 0): `pagina`..=`hasta` (como mucho
/// [`MAX_PAGINAS`]); sin `pagina`, la que el usuario tiene abierta.
fn paginas_pedidas(args: &Value, ruta: &Path, total: usize) -> Vec<usize> {
    if total == 0 {
        return Vec::new();
    }
    let abierta = crate::anotado_del_adjunto::leer(ruta).pagina.max(0.0).floor() as usize;
    let desde = entero(args, "pagina").filter(|p| *p >= 1).map(|p| p - 1).unwrap_or(abierta).min(total - 1);
    let hasta = entero(args, "hasta").filter(|h| *h >= 1).map(|h| h - 1).unwrap_or(desde).clamp(desde, total - 1);
    (desde..=hasta.min(desde + MAX_PAGINAS - 1)).collect()
}

// ---------------------------------------------------------------------------
// De letras con su caja a renglones y palabras

/// **Las palabras y renglones de una hoja leida** (letra a letra, cajas en
/// fracciones de la hoja) en las unidades de su capa (`ancho` x `alto`).
/// Una palabra acaba en un espacio o donde la letra salta de renglon; un
/// renglon, en un salto de linea o donde la letra baja (o vuelve atras).
pub fn renglones_de(t: &PaginaDeTexto, ancho: f32, alto: f32) -> Vec<Renglon> {
    let mut renglones: Vec<Renglon> = Vec::new();
    let mut actual = Renglon::default();
    let mut palabra = String::new();
    let mut caja: Option<[f32; 4]> = None;
    fn cerrar_palabra(actual: &mut Renglon, palabra: &mut String, caja: &mut Option<[f32; 4]>) {
        if let Some(c) = caja.take()
            && !palabra.trim().is_empty()
        {
            actual.palabras.push(Palabra { texto: std::mem::take(palabra), caja: c });
        }
        palabra.clear();
    }
    fn cerrar_renglon(renglones: &mut Vec<Renglon>, actual: &mut Renglon) {
        if !actual.palabras.is_empty() {
            renglones.push(std::mem::take(actual));
        }
    }
    for (k, ch) in t.texto.chars().enumerate() {
        if ch == '\n' {
            cerrar_palabra(&mut actual, &mut palabra, &mut caja);
            cerrar_renglon(&mut renglones, &mut actual);
            continue;
        }
        let Some(Some(f)) = t.cajas.get(k) else {
            cerrar_palabra(&mut actual, &mut palabra, &mut caja);
            continue;
        };
        if ch.is_whitespace() {
            cerrar_palabra(&mut actual, &mut palabra, &mut caja);
            continue;
        }
        let c = [f[0] * ancho, f[1] * alto, f[2] * ancho, f[3] * alto];
        // ¿Sigue en el mismo renglon? Mirando a lo ultimo que hay en el.
        let antes = caja.or_else(|| actual.palabras.last().map(|p| p.caja));
        if let Some(u) = antes {
            let h = (u[3] - u[1]).max(c[3] - c[1]).max(1e-3);
            let salta = ((c[1] + c[3]) / 2.0 - (u[1] + u[3]) / 2.0).abs() > h * 0.6 || c[0] < u[0] - h * 2.0;
            if salta {
                cerrar_palabra(&mut actual, &mut palabra, &mut caja);
                cerrar_renglon(&mut renglones, &mut actual);
            }
        }
        palabra.push(ch);
        caja = Some(match caja {
            None => c,
            Some(u) => [u[0].min(c[0]), u[1].min(c[1]), u[2].max(c[2]), u[3].max(c[3])],
        });
    }
    cerrar_palabra(&mut actual, &mut palabra, &mut caja);
    cerrar_renglon(&mut renglones, &mut actual);
    renglones
}

/// La mediana del alto de las palabras (0 sin palabras).
fn alto_tipico(renglones: &[Renglon]) -> f32 {
    let mut h: Vec<f32> = renglones.iter().flat_map(|r| r.palabras.iter().map(|p| p.caja[3] - p.caja[1])).filter(|h| *h > 0.0).collect();
    if h.is_empty() {
        return 0.0;
    }
    h.sort_by(f32::total_cmp);
    h[h.len() / 2]
}

/// El OCR de una imagen como renglones (sus lineas) en pixeles de la imagen
/// por `escala` (para volver a los de la original si se redujo).
fn renglones_de_ocr(rgba: &[u8], ancho: u32, alto: u32, escala: f32) -> Result<Vec<Renglon>, String> {
    if !pixpin_ocr::disponible() {
        return Err("el OCR de Windows no esta disponible (falta el idioma en Windows)".into());
    }
    let lineas = pixpin_ocr::reconocer(ancho, alto, rgba).map_err(|e| format!("el OCR fallo: {e}"))?;
    Ok(lineas
        .iter()
        .map(|l| Renglon {
            palabras: l
                .palabras
                .iter()
                .map(|p| {
                    let (x, y, w, h) = (p.caja.x as f32, p.caja.y as f32, p.caja.ancho as f32, p.caja.alto as f32);
                    Palabra { texto: p.texto.clone(), caja: [x * escala, y * escala, (x + w) * escala, (y + h) * escala] }
                })
                .collect(),
            etiqueta: None,
        })
        .filter(|r| !r.palabras.is_empty())
        .collect())
}

/// La letra de las notas sobre una hoja de 1400 de ancho: algo menos que la
/// del texto (se ve que es una nota), sin bajar de lo que se lee a la hoja
/// entera en pantalla.
fn letra_de_hoja(renglones: &[Renglon]) -> f32 {
    let t = alto_tipico(renglones);
    if t <= 0.0 { 20.0 } else { (t * 0.72).clamp(17.0, 24.0) }
}

// ---------------------------------------------------------------------------
// Leer cada formato

/// **Lee lo que se va a anotar**, con donde cae cada palabra, en las
/// unidades de su capa.
pub fn leer(raiz: &Path, ruta: &Path, args: &Value, parar: &AtomicBool) -> Result<Leido, String> {
    let t = Instant::now();
    let formato = formato_de(ruta).ok_or("ese archivo no se puede anotar (PDF, Word, PowerPoint, Excel, foto o plano)")?;
    let (paginas, destino, con_ocr) = match formato {
        Formato::Pdf => leer_pdf(ruta, args, parar)?,
        Formato::Office => leer_office(ruta, args)?,
        Formato::Foto => leer_foto(ruta)?,
        Formato::Plano => leer_plano(raiz, ruta)?,
        Formato::Libro => leer_libro(ruta, args)?,
    };
    Ok(Leido { formato, paginas, destino, ms_leer: t.elapsed().as_millis(), con_ocr })
}

type Lectura = (Vec<Pagina>, Destino, usize);

fn pagina_de_hoja(numero: usize, alto: f32, renglones: Vec<Renglon>) -> Pagina {
    let letra = letra_de_hoja(&renglones);
    Pagina {
        numero,
        caja: [0.0, 0.0, ANCHO_HOJA, alto],
        renglones,
        letra,
        // El espacio para anotar del lector (`MARGEN_DEL_PDF`).
        fuera: ANCHO_HOJA * pixpin_docs::vista::MARGEN_DEL_PDF,
        solo_derecha: false,
    }
}

fn leer_pdf(ruta: &Path, args: &Value, parar: &AtomicBool) -> Result<Lectura, String> {
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir el PDF: {e}"))?;
    let altos: Vec<f32> = doc.medidas().iter().map(|(w, h)| ANCHO_HOJA * h / w.max(1.0)).collect();
    let pedidas = paginas_pedidas(args, ruta, altos.len());
    let textos = pixpin_pdf::texto::de_fichero(ruta).unwrap_or_default();
    let mut paginas = Vec::new();
    let mut con_ocr = 0;
    for i in pedidas {
        let alto = altos[i];
        let t = textos.get(i).cloned().unwrap_or_default();
        let renglones = if t.texto.trim().chars().count() >= 20 {
            renglones_de(&t, ANCHO_HOJA, alto)
        } else if con_ocr < MAX_OCR {
            // Escaneada: la hoja pintada, por el OCR, a las unidades de la capa.
            if parar.load(Ordering::Relaxed) {
                return Err("el usuario lo paro".into());
            }
            con_ocr += 1;
            let img = doc.renderizar(i as u32, ANCHO_OCR).map_err(|e| e.to_string())?;
            renglones_de_ocr(&img.pixeles, img.ancho, img.alto, ANCHO_HOJA / img.ancho.max(1) as f32)?
        } else {
            Vec::new()
        };
        paginas.push(pagina_de_hoja(i, alto, renglones));
    }
    let altos = paginas.iter().map(|p| (p.numero, p.caja[3])).collect();
    Ok((paginas, Destino::Hojas { altos }, con_ocr))
}

fn leer_office(ruta: &Path, args: &Value) -> Result<Lectura, String> {
    let mut doc = pixpin_office::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir: {e:?}"))?;
    let letras = pixpin_render::pagina_office::LetrasOffice::sin_motor().ok_or("sin DirectWrite")?;
    // Cuantas hojas hay no se sabe sin maquetar: hasta la pedida.
    let quiere = entero(args, "hasta").or(entero(args, "pagina")).unwrap_or(1).max(1);
    let abierta = crate::anotado_del_adjunto::leer(ruta).pagina.max(0.0).floor() as usize + 1;
    let hasta = quiere.max(if entero(args, "pagina").is_none() { abierta } else { 1 });
    let mut hojas = Vec::new();
    for i in 0..hasta.min(400) {
        let Some(p) = doc.pagina(i, &letras) else { break };
        hojas.push(p);
    }
    let pedidas = paginas_pedidas(args, ruta, hojas.len());
    if pedidas.is_empty() {
        return Err("el documento no tiene paginas".into());
    }
    let paginas: Vec<Pagina> = pedidas
        .into_iter()
        .map(|i| {
            let p = &hojas[i];
            let alto = ANCHO_HOJA * p.alto / p.ancho.max(1.0);
            let t = crate::lector_pdf::office::texto_de_pagina(p, &letras);
            pagina_de_hoja(i, alto, renglones_de(&t, ANCHO_HOJA, alto))
        })
        .collect();
    let altos = paginas.iter().map(|p| (p.numero, p.caja[3])).collect();
    Ok((paginas, Destino::Hojas { altos }, 0))
}

fn leer_foto(ruta: &Path) -> Result<Lectura, String> {
    let img = pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
    let (w, h) = (img.ancho, img.alto);
    let mayor = w.max(h);
    let (img, escala) = if mayor > LADO_OCR {
        let k = LADO_OCR as f32 / mayor as f32;
        let (a, b) = (((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1));
        (pixpin_codec::redimensionar(img, a, b).map_err(|e| format!("no se pudo reducir la imagen: {e:?}"))?, 1.0 / k)
    } else {
        (img, 1.0)
    };
    let renglones = renglones_de_ocr(&img.pixeles, img.ancho, img.alto, escala)?;
    let (wf, hf) = (w as f32, h as f32);
    let t = alto_tipico(&renglones);
    // La letra del texto de la foto, sin bajar de lo que se lee con la foto
    // entera en pantalla (unos 900 px de ancho: 12 px).
    let letra = (t * 0.8).max(wf / 75.0).max(10.0);
    let pagina = Pagina { numero: 0, caja: [0.0, 0.0, wf, hf], renglones, letra, fuera: (wf * 0.6).max(letra * 20.0), solo_derecha: false };
    let mut dibujo = ruta.as_os_str().to_owned();
    dibujo.push(".pixpin2d");
    Ok((vec![pagina], Destino::Foto { dibujo: PathBuf::from(dibujo) }, 1))
}

fn leer_plano(raiz: &Path, ruta: &Path) -> Result<Lectura, String> {
    use crate::plano_cad::anotar;
    let m = crate::plano_cad::modelo(raiz, ruta)?;
    let rutas = anotar::rutas_de_la_capa(raiz, ruta);
    // El encaje con que se escribe (el del fichero): se le pregunta a
    // `anadir_figuras` sin anadir nada.
    let mut encaje = None;
    anotar::anadir_figuras(&rutas, &m, |e| {
        encaje = Some(*e);
        Vec::new()
    })
    .map_err(|e| e.to_string())?;
    let encaje = encaje.ok_or("sin encaje del plano")?;
    let o = m.origen;
    let a_capa = |x: f64, y: f64| {
        let q = encaje.a_capa([x, y]);
        (q[0] as f32, q[1] as f32)
    };
    let mut textos = crate::plano_cad::textos::del_plano(raiz, ruta)?;
    // Los rotulos grandes primero (los que dicen que es cada cosa), sin
    // repetir el mismo texto mas de dos veces.
    textos.sort_by(|a, b| b.alto.total_cmp(&a.alto));
    let mut vistos: HashMap<String, usize> = HashMap::new();
    let mut renglones = Vec::new();
    for t in &textos {
        let clave = pixpin_ia::acciones::normalizar(t.texto.trim());
        if clave.is_empty() || t.alto <= 0.0 {
            continue;
        }
        let n = vistos.entry(clave).or_default();
        *n += 1;
        if *n > 2 {
            continue;
        }
        let (x, y) = (t.x - o[0], t.y - o[1]);
        let letras = t.texto.chars().count().max(1) as f64;
        let largo = letras * t.alto * 0.75;
        // Cada palabra, en su parte del rotulo.
        let mut palabras = Vec::new();
        let mut desde = 0usize;
        for w in t.texto.split_whitespace() {
            let i = desde + t.texto[desde..].find(w).unwrap_or(0);
            let ini = t.texto[..i].chars().count() as f64;
            let fin = ini + w.chars().count() as f64;
            desde = i + w.len();
            let (x0, y0) = a_capa(x + largo * ini / letras, y + t.alto);
            let (x1, y1) = a_capa(x + largo * fin / letras, y);
            palabras.push(Palabra { texto: w.to_string(), caja: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)] });
        }
        if !palabras.is_empty() {
            renglones.push(Renglon { palabras, etiqueta: None });
        }
    }
    let c = m.caja_util();
    let (x0, y0) = a_capa(c[0] as f64, c[3] as f64);
    let (x1, y1) = a_capa(c[2] as f64, c[1] as f64);
    let caja = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
    let (ancho, alto) = (caja[2] - caja[0], caja[3] - caja[1]);
    let t = alto_tipico(&renglones);
    let letra = (t * 2.0).max(alto.max(ancho) / 90.0).max(1e-3);
    let pagina = Pagina { numero: 0, caja, renglones, letra, fuera: (ancho * 0.5).max(letra * 20.0), solo_derecha: false };
    Ok((vec![pagina], Destino::Plano { rutas, modelo: Box::new(m) }, 0))
}

/// La rejilla de la capa de una hoja de calculo: pixeles al 100 % a 96 ppp
/// con el ancho de digito de la tabla (sin DirectWrite). El visor la
/// rehace igual y lleva cada punto a su celda (`visor_hoja::capa`).
pub fn rejilla_de_la_capa(libro: &pixpin_hojas::Libro, hoja: &pixpin_hojas::Hoja) -> pixpin_hojas::Rejilla {
    pixpin_hojas::Rejilla::nueva(hoja, &libro.estilos.fuente_normal)
}

fn leer_libro(ruta: &Path, args: &Value) -> Result<Lectura, String> {
    use pixpin_hojas::{Libro, Regional, nombre_columna};
    let libro = Libro::abrir(ruta, Regional::es_pe()).map_err(|e| format!("no se pudo abrir el libro: {e:?}"))?;
    let i = match texto_de(args, "hoja") {
        Some(n) => libro
            .hojas
            .iter()
            .position(|h| pixpin_ia::acciones::normalizar(&h.nombre) == pixpin_ia::acciones::normalizar(n))
            .ok_or_else(|| format!("el libro no tiene una hoja «{n}»"))?,
        None => 0,
    };
    let mut hoja = libro.abrir_hoja(i);
    let rango = texto_de(args, "celdas").and_then(pixpin_ia::anotacion::ancla::rango);
    let hasta_fila = rango.map_or(MAX_FILAS, |r| (r.1.0 + 1).min(MAX_FILAS * 4));
    hoja.cargar_hasta_fila(hasta_fila);
    let rejilla = rejilla_de_la_capa(&libro, &hoja);
    let mut renglones = Vec::new();
    let (mut max_f, mut max_c) = (0u32, 0u32);
    'filas: for f in &hoja.filas {
        if f.oculta || f.indice > hasta_fila {
            continue;
        }
        for c in hoja.celdas_de(f) {
            if let Some(((f0, c0), (f1, c1))) = rango
                && !((f0..=f1).contains(&f.indice) && (c0..=c1).contains(&c.col))
            {
                continue;
            }
            let Some(texto) = libro.contenido(&hoja, c).map(|x| x.mostrado.texto).filter(|t| !t.trim().is_empty()) else { continue };
            let r = hoja.combinada_en(f.indice, c.col).unwrap_or(pixpin_hojas::Rango { fila0: f.indice, col0: c.col, fila1: f.indice, col1: c.col });
            let (x0, y0) = (rejilla.x(r.col0) as f32, rejilla.y(r.fila0) as f32);
            let (x1, y1) = (rejilla.x(r.col1 + 1) as f32, rejilla.y(r.fila1 + 1) as f32);
            // Las palabras repartidas en la celda (un ancla de texto cae en
            // su celda, que es lo que importa).
            let total = texto.chars().count().max(1) as f32;
            let mut palabras = Vec::new();
            let mut pos = 0.0f32;
            for w in texto.split_whitespace() {
                let n = w.chars().count() as f32;
                let (a, b) = (x0 + (x1 - x0) * pos / total, x0 + (x1 - x0) * ((pos + n) / total).min(1.0));
                palabras.push(Palabra { texto: w.to_string(), caja: [a, y0, b.max(a + 1.0), y1] });
                pos += n + 1.0;
            }
            renglones.push(Renglon { palabras, etiqueta: Some(format!("{}{}", nombre_columna(c.col), f.indice + 1)) });
            max_f = max_f.max(r.fila1);
            max_c = max_c.max(r.col1);
            if renglones.len() >= MAX_CELDAS {
                break 'filas;
            }
        }
    }
    // Las notas, a la derecha de TODO lo usado de la hoja (no solo del rango
    // mirado): si no, taparian columnas con datos.
    let usado = rejilla.tamano_usado(&hoja).0.max(rejilla.x(max_c + 1));
    let caja = [0.0, 0.0, usado as f32, rejilla.y(max_f + 1) as f32];
    let letra = 12.0;
    // Las notas, a la derecha de lo usado (a la izquierda no hay hoja).
    let pagina = Pagina { numero: 0, caja, renglones, letra, fuera: letra * 22.0, solo_derecha: true };
    Ok((vec![pagina], Destino::Libro { hoja: i }, 0))
}

// ---------------------------------------------------------------------------
// Escribir en la capa

impl Destino {
    /// **Anade `nuevas` a la capa de la pagina `numero`** y avisa a los
    /// visores. Devuelve el fichero de la capa.
    pub fn anadir(&self, raiz: &Path, ruta: &Path, numero: usize, grupo: &str, nuevas: Vec<Elemento>) -> Result<PathBuf, String> {
        use crate::lector_tinta::turnos;
        let capa = match self {
            Destino::Hojas { altos } => {
                let alto = altos.get(&numero).copied().unwrap_or(ANCHO_HOJA * 1.414);
                let d = crate::lector_pdf_proyecto::DondeVa::de(raiz, ruta, pixpin_shell::entorno::ahora_utc_ms());
                // Antes de escribir: el visor que relea ya sabe que es del turno.
                let (leer, escribir) = (d.para_leer(ruta, numero), d.para_escribir(ruta, numero));
                turnos::apuntar(&leer, grupo);
                turnos::apuntar(&escribir, grupo);
                super::documentos_acciones::escribir_en_la_hoja(raiz, ruta, numero, alto, nuevas)?;
                escribir
            }
            Destino::Foto { dibujo } => {
                turnos::apuntar(dibujo, grupo);
                let mut escena = pixpin_motor2d::cargar(dibujo).map_err(|e| format!("el dibujo de la foto no se entiende: {e}"))?;
                for e in nuevas {
                    escena.anadir(e);
                }
                pixpin_motor2d::guardar(dibujo, &escena).map_err(|e| format!("no se pudo guardar lo anotado: {e}"))?;
                dibujo.clone()
            }
            Destino::Plano { rutas, modelo } => {
                turnos::apuntar(&rutas.tinta, grupo);
                crate::plano_cad::anotar::anadir_figuras(rutas, modelo, |_| nuevas).map_err(|e| format!("no se pudo guardar lo anotado en el plano: {e}"))?;
                rutas.tinta.clone()
            }
            Destino::Libro { hoja } => {
                let r = crate::lector_tinta::ruta_de_hoja(ruta, *hoja);
                turnos::apuntar(&r, grupo);
                let mut capa = crate::lector_tinta::Capa::leer(&r);
                for e in nuevas {
                    capa.escena.anadir(e);
                }
                capa.sucia = true;
                capa.guardar(&r).map_err(|e| format!("no se pudo guardar lo anotado: {e}"))?;
                r
            }
        };
        turnos::avisar();
        Ok(capa)
    }
}

/// Abre el espacio para anotar del lado `lado` de un PDF o un Word: en su
/// `.pixpin-lectura` (lo que vera al abrir) y en el lector si esta abierto.
fn abrir_espacio(raiz: &Path, ruta: &Path, lado: u8, altos: &[f32]) {
    let mut a = crate::anotado_del_adjunto::leer(ruta);
    if a.espacios & lado == 0 {
        // La tinta de antes del marco no se corre (`alternar_espacio`).
        crate::lector_pdf_proyecto::DondeVa::de(raiz, ruta, pixpin_shell::entorno::ahora_utc_ms()).fijar_lo_viejo(ruta, a.espacios, altos);
        a.espacios |= lado;
        if let Err(e) = crate::anotado_del_adjunto::escribir(ruta, &a) {
            tracing::info!(?e, "anotar: no se pudo abrir el espacio para anotar");
        }
    }
    crate::lector_pdf::pedir_espacio(ruta, lado);
}

// ---------------------------------------------------------------------------
// El turno entero

/// Lo que se hizo (para el modelo, el banco y la revision visual).
#[derive(Debug, Clone, Default)]
pub struct Hecho {
    pub resumen: String,
    pub puestas: usize,
    /// El grupo de este turno (`asistente-anotado:<turno>`).
    pub grupo: String,
    /// Los ficheros de capa tocados, con su pagina (desde 0).
    pub capas: Vec<(usize, PathBuf)>,
    /// Las paginas leidas (desde 0).
    pub paginas: Vec<usize>,
    pub ms_leer: u128,
    /// Desde que llego el primer trozo de la respuesta hasta la primera
    /// anotacion escrita, y lo que tarda poner una (anclar, colocar,
    /// escribir) como mucho.
    pub ms_a_la_primera: Option<u128>,
    pub ms_poner_max: u128,
    /// De eso, lo que tarda anclar, colocar y hacer las figuras (sin escribir).
    pub ms_anclar_max: u128,
    pub ms_total: u128,
    pub con_ocr: usize,
    /// Lo que dijo el modelo si no habia nada que anotar.
    pub nada: Option<String>,
}

/// El pedido para el modelo: lo que dijo el usuario, el modo y el foco.
fn pedido_de(args: &Value) -> String {
    let mut s = texto_de(args, "pedido").unwrap_or("señala lo importante").to_string();
    if let Some(m) = texto_de(args, "modo").filter(|m| *m != "mixto") {
        s.push_str(&format!(" (modo: {m})"));
    }
    if let Some(f) = texto_de(args, "foco") {
        s.push_str(&format!(". Lo que hay que explicar: «{f}» (anclalo ahi)"));
    }
    s
}

/// **Anota sin mandar nada** (el banco y las pruebas lo llaman asi): lee,
/// pregunta al `motor`, y pone cada anotacion en la capa segun llega.
/// `al_poner(primera)` se llama tras escribir cada una (la primera abre el
/// visor).
pub fn anotar_en(
    raiz: &Path,
    ruta: &Path,
    args: &Value,
    motor: &dyn Motor,
    parar: &AtomicBool,
    al_poner: &mut dyn FnMut(&Hecho, bool),
) -> Result<Hecho, String> {
    let t0 = Instant::now();
    let leido = leer(raiz, ruta, args, parar)?;
    if leido.paginas.iter().all(Pagina::vacia) {
        return Err(match leido.formato {
            Formato::Foto | Formato::Pdf if leido.con_ocr > 0 => "en esa pagina no se lee texto (ni con el OCR): no hay nada que anotar".into(),
            _ => "esa pagina no tiene texto que anotar".into(),
        });
    }
    let turno = format!("{}", pixpin_shell::entorno::ahora_utc_ms());
    let mut anotador = Anotador::nuevo(leido.paginas.clone(), &turno);
    let mut hecho = Hecho {
        grupo: anotador.grupo(),
        paginas: leido.paginas.iter().map(|p| p.numero).collect(),
        ms_leer: leido.ms_leer,
        con_ocr: leido.con_ocr,
        ..Default::default()
    };
    let texto = anotacion::mensaje(&pedido_de(args), &leido.paginas, anotacion::MAX_LETRAS);
    // Una foto, con la foto (banco de documentos, 15-oct): el OCR leyo
    // «mdAC = 350» por «m∢BAC = 35°» y el anotador corrigio un error que no
    // estaba. Con la imagen delante entiende la hoja; ancla igual por el
    // texto leido.
    let foto = (leido.formato == Formato::Foto && motor.ve_imagenes())
        .then(|| super::foto::preparar_fichero(ruta).ok())
        .flatten();
    let usuario = match foto {
        Some((bytes, mime)) => Mensaje::usuario_con_imagen(
            format!(
                "{texto}\n\n(La imagen adjunta es la hoja original. El texto de arriba sale del OCR y puede traer letras mal leidas \
                 (simbolos, grados, subindices): entiende la hoja por la imagen, pero cita el texto TAL COMO lo leyo el OCR para anclar.)"
            ),
            bytes,
            mime,
        ),
        None => Mensaje::usuario(texto),
    };
    let peticion = Peticion {
        mensajes: vec![Mensaje::sistema(anotacion::instrucciones(leido.formato == Formato::Libro)), usuario],
        herramientas: Vec::new(),
        max_tokens: 3000,
        temperatura: 0.3,
        pensar: false,
    };
    let mut flujo = anotacion::flujo::Flujo::nuevo();
    let mut primer_trozo: Option<Instant> = None;
    let mut trozos = 0usize;
    let mut fallo: Option<String> = None;
    let mut espacios: u8 = 0;
    let mut poner = |v: Value, hecho: &mut Hecho, primer_trozo: Option<Instant>| {
        let p = match Leida::de(&v) {
            Leida::Anotacion(p) => p,
            Leida::Nada(m) => {
                hecho.nada = Some(m);
                return;
            }
            Leida::Mal(m) => {
                tracing::info!(%m, "anotar: una anotacion que no se entiende");
                return;
            }
        };
        let t = Instant::now();
        let puesta = match anotador.poner(&p) {
            Ok(x) => x,
            Err(m) => {
                tracing::info!(%m, "anotar: no se puso");
                return;
            }
        };
        let ms_anclar = t.elapsed().as_millis();
        hecho.ms_anclar_max = hecho.ms_anclar_max.max(ms_anclar);
        let numero = anotador.paginas[puesta.en].numero;
        match leido.destino.anadir(raiz, ruta, numero, &hecho.grupo, puesta.elementos) {
            Ok(capa) => {
                if !hecho.capas.iter().any(|(n, c)| *n == numero && *c == capa) {
                    hecho.capas.push((numero, capa));
                }
            }
            Err(m) => {
                fallo.get_or_insert(m);
                return;
            }
        }
        // Una nota fuera de la hoja de un PDF o un Word: su espacio, abierto.
        if puesta.fuera && matches!(leido.destino, Destino::Hojas { .. }) {
            let lado = match puesta.lado {
                anotacion::colocar::Lado::Izquierda => pixpin_docs::vista::ESPACIO_IZQUIERDA,
                _ => pixpin_docs::vista::ESPACIO_DERECHA,
            };
            if espacios & lado == 0 {
                espacios |= lado;
                let altos: Vec<f32> = leido.paginas.iter().map(|p| p.caja[3]).collect();
                abrir_espacio(raiz, ruta, lado, &altos);
            }
        }
        hecho.ms_poner_max = hecho.ms_poner_max.max(t.elapsed().as_millis());
        hecho.puestas = anotador.cuantas();
        let primera = hecho.puestas == 1;
        if primera {
            hecho.ms_a_la_primera = primer_trozo.map(|t| t.elapsed().as_millis());
        }
        al_poner(hecho, primera);
    };
    let respuesta = motor
        .completar(&peticion, &mut |t| {
            if let Trozo::Texto(s) = t {
                primer_trozo.get_or_insert_with(Instant::now);
                trozos += 1;
                for v in flujo.empujar(s) {
                    poner(v, &mut hecho, primer_trozo);
                }
            }
            if parar.load(Ordering::Relaxed) { Seguir::Parar } else { Seguir::Si }
        })
        .map_err(|e| format!("el modelo no contesto: {e}"))?;
    // Un motor que no da la respuesta a trozos: de una vez al final.
    if trozos == 0 && !respuesta.texto.is_empty() {
        primer_trozo = Some(Instant::now());
        for v in flujo.empujar(&respuesta.texto) {
            poner(v, &mut hecho, primer_trozo);
        }
    }
    drop(poner);
    if let Some(m) = fallo
        && anotador.cuantas() == 0
    {
        return Err(m);
    }
    hecho.resumen = anotador.resumen();
    hecho.puestas = anotador.cuantas();
    hecho.ms_total = t0.elapsed().as_millis();
    tracing::info!(
        puestas = hecho.puestas,
        ms_leer = hecho.ms_leer,
        ms_a_la_primera = ?hecho.ms_a_la_primera,
        ms_poner_max = hecho.ms_poner_max,
        ms_total = hecho.ms_total,
        "anotar_inteligente: hecho"
    );
    Ok(hecho)
}

/// El modelo del anotador: el de la nube de Ajustes (Claude o DeepSeek), en
/// modo ligero. El local no se toca (decision del usuario, 15-oct: primero
/// la nube): con el solo, la herramienta dice por que no ([`SOLO_NUBE`]).
pub fn motor_de_la_nube() -> Option<Arc<dyn Motor>> {
    super::hoja_a_lienzo::motor_de_vision()
}

/// Lo ultimo que anoto la herramienta en las pruebas (el documento, el grupo
/// del turno y las capas tocadas), para pintarlo en el banco de documentos.
#[cfg(test)]
pub static ULTIMO_EN_PRUEBAS: std::sync::Mutex<Option<(PathBuf, String, Vec<(usize, PathBuf)>)>> = std::sync::Mutex::new(None);

/// Lo que dice la herramienta cuando solo hay modelo local.
pub const SOLO_NUBE: &str = "anotar con criterio necesita el modelo de la nube (Claude o DeepSeek en Ajustes → Asistente); \
                             con el local puedo resaltar o encerrar un texto concreto con anotar_documento";

/// **La herramienta desde el ejecutor del chat**: elige el archivo, anota y
/// abre el visor en la pagina con la primera anotacion.
pub fn hacer(args: &Value, cx: &Contexto, motor: Option<Arc<dyn Motor>>) -> Result<String, String> {
    let Some(motor) = motor else { return Err(SOLO_NUBE.into()) };
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Anotable)?;
    let formato = formato_de(&c.ruta);
    let mut abierto = false;
    let mut al_poner = |h: &Hecho, primera: bool| {
        if !primera || abierto {
            return;
        }
        abierto = true;
        let mut que = json!({"tipo": "fichero", "ruta": c.ruta});
        if matches!(formato, Some(Formato::Pdf | Formato::Office))
            && let Some((p, _)) = h.capas.first()
        {
            que["pagina"] = json!(p + 1);
        }
        let _ = (cx.mandar)(&json!({"pixpin": 1, "accion": "abrir", "que": que}).to_string());
        if matches!(formato, Some(Formato::Foto)) {
            crate::ventana_chat::refrescar();
        }
    };
    let h = anotar_en(cx.raiz, &c.ruta, args, motor.as_ref(), cx.parar, &mut al_poner)?;
    #[cfg(test)]
    {
        *ULTIMO_EN_PRUEBAS.lock().unwrap_or_else(|e| e.into_inner()) = Some((c.ruta.clone(), h.grupo.clone(), h.capas.clone()));
    }
    if h.puestas == 0 {
        return Ok(match h.nada {
            Some(m) => format!("no anote nada en «{}»: {m}", c.nombre),
            None => format!("no pude anclar ninguna anotacion en «{}» ({})", c.nombre, h.resumen),
        });
    }
    let paginas = if matches!(formato, Some(Formato::Pdf | Formato::Office)) {
        let mut ps: Vec<usize> = h.capas.iter().map(|(p, _)| p + 1).collect();
        ps.sort_unstable();
        ps.dedup();
        format!(", pagina(s) {}", ps.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
    } else {
        String::new()
    };
    Ok(format!(
        "hecho: {} en «{}»{paginas}; ya esta abierto ahi, en la capa de anotaciones (viaja al movil; Ctrl+Z lo quita entero)",
        h.resumen, c.nombre
    ))
}
