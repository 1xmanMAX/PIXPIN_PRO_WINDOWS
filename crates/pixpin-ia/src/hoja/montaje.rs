//! **Cada zona, a elementos del lienzo** (en Excalidraw, como todo lo que
//! dibuja el asistente: `lienzo::Fabrica`), ya en su caja destino.
//!
//! - Parrafos y encabezados → **texto del lienzo**, con el tamano de letra
//!   que hace que cada renglon mida lo que media en el papel, su alineacion,
//!   negrita en los titulos y los mismos saltos de renglon. Si el
//!   interlineado del papel no es el de la letra, cada renglon va suelto en
//!   su sitio (agrupados), que si no el parrafo se estiraria.
//! - Tablas → **la tabla dibujada** del motor (`tabla_dibujada`), con la
//!   letra elegida para que ocupe el ancho de la del papel.
//! - Fotos → **la imagen recortada**, como una imagen pegada.
//! - Lineas → rectas lisas.
//! - Formulas → el LaTeX del modelo por **`escribir_ecuacion`**
//!   (`ecuacion::validar` + `ecuacion::piezas`), encajada en su caja.
//! - Dibujos (y graficas que no son funciones) → **calcado**
//!   (`calcar::calcar`), recolocado en su caja.
//! - Graficas de funciones → **`graficar_funcion`** (`funciones::piezas`)
//!   en su caja.
//!
//! Cada zona sale en su grupo (se mueve de una vez y se desagrupa para
//! tocar una pieza). Lo que la comprobacion rehace va por otra [`Via`].

use serde_json::{Map, Value, json};

use pixpin_motor2d::ecuacion::{Estilo, elemento_texto};
use pixpin_motor2d::elemento::{ColorRgba, Elemento};
use pixpin_motor2d::texto::{AlineacionTexto, EstiloDeTexto};
use pixpin_motor2d::vector::Punto2;

use super::{Alineacion, Caja, Mapa, Opciones, Renglon, Tipo, Zona, imagen, mediana};
use crate::lienzo::Fabrica;

/// Como se hace una zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Lo normal: lo mas editable (texto corrido, tabla dibujada, ecuacion).
    Editable,
    /// Cada renglon (o trozo de tabla) en su sitio exacto, y las rayas tal
    /// cual: lo que se usa si lo editable no se parece.
    Fiel,
    /// El recorte de la imagen: lo ultimo, cuando nada se parece.
    Imagen,
}

/// Lo que el modelo leyo de un recorte.
#[derive(Debug, Clone, PartialEq)]
pub enum Leido {
    /// Una formula, en LaTeX (una linea por renglon).
    Latex(Vec<String>),
    /// Lo que es una zona dudosa.
    Tipo(Tipo),
    /// Una grafica de funciones (los argumentos de `graficar_funcion`).
    Funciones(Map<String, Value>),
}

/// Una imagen que nace con la zona: va al almacen del lienzo con este id
/// (su `fileId` es `pc<hex>`).
#[derive(Debug, Clone, PartialEq)]
pub struct ImagenNueva {
    pub id: u64,
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
}

/// Una zona hecha.
#[derive(Debug, Clone, PartialEq)]
pub struct Hecho {
    pub zona: usize,
    pub tipo: Tipo,
    pub via: Via,
    pub elementos: Vec<Value>,
    pub imagenes: Vec<ImagenNueva>,
    /// Lo que se hizo, en corto (para el registro y el banco).
    pub nota: String,
}

/// Lo que recibe el trabajador de una zona.
pub struct Entrada<'a> {
    pub zona: &'a Zona,
    pub mapa: Mapa,
    pub opciones: &'a Opciones,
    /// La imagen de la pagina (RGBA, ancho, alto).
    pub pagina: (&'a [u8], u32, u32),
    /// El alto de letra del cuerpo, en pixeles de la pagina.
    pub alto_letra: f64,
    pub semilla: u64,
}

impl Entrada<'_> {
    pub fn destino(&self) -> Caja {
        self.mapa.caja(&self.zona.caja)
    }
    /// **La semilla de ESTA zona**: la de la hoja mezclada con el numero de
    /// zona. Con la misma semilla en todas, los ids (`ia-<semilla>-<n>`) se
    /// repetian de una zona a otra y rehacer una (que borra por id) se
    /// llevaba piezas de las demas (banco del 15-oct, cuadratica 32/65).
    pub fn semilla_de_zona(&self) -> u64 {
        id_de_imagen(self.semilla, 1_000_000 + self.zona.id)
    }
    fn fabrica(&self) -> Fabrica {
        let mut f = Fabrica::nueva((0.0, 0.0), self.semilla_de_zona());
        f.grupo = Some(f.nombre_de_grupo(&format!("hoja-{}", self.zona.id)));
        f
    }
    /// El recorte de la zona (RGBA, ancho, alto).
    pub fn recorte(&self) -> (Vec<u8>, u32, u32) {
        let (rgba, w, h) = self.pagina;
        imagen::recortar(rgba, w, h, &self.zona.caja)
    }
}

const NEGRO: &str = "#1e1e1e";

fn negro() -> ColorRgba {
    ColorRgba::opaco(0.118, 0.118, 0.118)
}

/// **Hace una zona.** `leido`: lo que dijo el modelo (formulas, dudosas,
/// graficas); sin el, lo que se pueda sin modelo.
pub fn hacer(e: &Entrada, leido: Option<&Leido>, via: Via) -> Hecho {
    let z = e.zona;
    let tipo = match (z.tipo, leido) {
        // Lo que dijo el modelo de una dudosa, o de un dibujo o foto por los
        // que pregunto la app (antes la respuesta de estos se perdia y un
        // sistema de ecuaciones escaneado salia calcado).
        (Tipo::Dudosa | Tipo::Dibujo | Tipo::Foto, Some(Leido::Tipo(t))) => *t,
        (Tipo::Dudosa | Tipo::Dibujo | Tipo::Foto, Some(Leido::Latex(_))) => Tipo::Formula,
        (t, _) => t,
    };
    let mut imagenes = Vec::new();
    let (elementos, nota) = match (via, tipo) {
        (Via::Imagen, _) | (_, Tipo::Foto) => {
            let (els, img) = foto(e);
            imagenes.extend(img);
            (els, "imagen recortada".to_string())
        }
        (_, Tipo::Linea) => (linea(e), "recta".into()),
        // Una tabla con iconos en sus celdas: cada cosa en su sitio y los
        // iconos recortados encima (la tabla dibujada no lleva imagenes).
        (_, Tipo::Tabla) if !z.dentro.is_empty() => {
            let mut els = fiel(e);
            for (k, c) in z.dentro.iter().enumerate() {
                let grupo = els.first().map(|v| v["groupIds"].clone());
                if let Some((v, img)) = foto_de_caja(e, c, k + 1, grupo) {
                    els.push(v);
                    imagenes.push(img);
                }
            }
            (els, format!("tabla en su sitio con {} imagen(es)", z.dentro.len()))
        }
        (Via::Fiel, Tipo::Parrafo | Tipo::Encabezado | Tipo::Tabla) => (fiel(e), "renglones en su sitio".into()),
        (_, Tipo::Parrafo | Tipo::Encabezado) => (texto(e, tipo == Tipo::Encabezado), "texto".into()),
        (_, Tipo::Tabla) => match tabla(e) {
            Some(els) => (els, "tabla dibujada".into()),
            None => (fiel(e), "tabla sin celdas: renglones en su sitio".into()),
        },
        (_, Tipo::Formula) => match leido {
            Some(Leido::Latex(l)) if !l.is_empty() => match formula(e, l) {
                Some(els) => (els, format!("ecuacion: {}", l.join(" \\\\ "))),
                None => {
                    let (els, img) = foto(e);
                    imagenes.extend(img);
                    (els, "formula ilegible: imagen".into())
                }
            },
            // Sin LaTeX (sin modelo o fallo): la formula como imagen, que
            // se ve igual que en el papel.
            _ => {
                let (els, img) = foto(e);
                imagenes.extend(img);
                (els, "formula sin leer: imagen".into())
            }
        },
        (_, Tipo::Grafica) => match leido {
            Some(Leido::Funciones(args)) => match grafica(e, args) {
                Some(els) => (els, "grafica de funciones".into()),
                None => (calco(e), "grafica calcada".into()),
            },
            _ => (calco(e), "grafica calcada".into()),
        },
        (_, Tipo::Dibujo) => (calco(e), "dibujo calcado".into()),
        (_, Tipo::Dudosa) => {
            // Sin modelo que lo diga: como imagen, que es lo que menos
            // inventa.
            let (els, img) = foto(e);
            imagenes.extend(img);
            (els, "dudosa: imagen".into())
        }
    };
    // Nada salio (un «texto» sin palabras que dijo el modelo, una tabla
    // vacia, un calco en blanco): el recorte, que algo hay.
    if elementos.is_empty() && via != Via::Imagen {
        let mut h = hacer(e, None, Via::Imagen);
        h.tipo = tipo;
        h.nota = format!("{nota}: no salio nada, imagen");
        return h;
    }
    Hecho { zona: z.id, tipo, via, elementos, imagenes, nota }
}

// ---------------------------------------------------------------------------
// Texto

/// Lo que mide un texto con su letra (y su negrita, si hay medidor).
fn medir(t: &str, tam: f32, familia: &str, negrita: bool) -> (f32, f32) {
    if pixpin_motor2d::texto::hay_medidor() {
        pixpin_motor2d::texto::medida(t, tam, familia, EstiloDeTexto { negrita, ..Default::default() })
    } else {
        crate::lienzo::medir_con(t, tam, familia)
    }
}

/// **El tamano de letra** que hace que los renglones midan lo que median:
/// la mediana de lo que pide cada renglon largo, sujeta a lo que dice su
/// alto (un renglon con pocas letras anchas no manda).
pub fn tam_de_letra(rs: &[Renglon], escala: f64, familia: &str, negrita: bool) -> f64 {
    let por_alto = mediana(rs.iter().map(|r| r.alto() * escala * 1.05).collect()).unwrap_or(16.0).max(4.0);
    let mut por_ancho: Vec<f64> = rs
        .iter()
        .filter(|r| r.texto.chars().count() >= 10)
        .filter_map(|r| {
            let (w, _) = medir(&r.texto, 100.0, familia, negrita);
            (w > 1.0).then(|| 100.0 * r.caja.w * escala / w as f64)
        })
        .collect();
    if por_ancho.is_empty() {
        por_ancho = rs
            .iter()
            .filter_map(|r| {
                let (w, _) = medir(&r.texto, 100.0, familia, negrita);
                (w > 1.0).then(|| 100.0 * r.caja.w * escala / w as f64)
            })
            .collect();
    }
    let t = mediana(por_ancho).unwrap_or(por_alto).clamp(por_alto * 0.7, por_alto * 1.45);
    (t * 2.0).round() / 2.0
}

fn alineacion_motor(a: Alineacion) -> AlineacionTexto {
    match a {
        Alineacion::Centro => AlineacionTexto::Centro,
        Alineacion::Derecha => AlineacionTexto::Derecha,
        Alineacion::Izquierda | Alineacion::Justificada => AlineacionTexto::Izquierda,
    }
}

/// El enlace de un texto (una direccion que empieza por `http`, `www.` o
/// tiene un dominio con barra), sin la puntuacion de alrededor.
pub fn enlace_de(t: &str) -> Option<String> {
    let w = t.split_whitespace().find(|w| {
        let w = w.to_lowercase();
        w.starts_with("http") || w.starts_with("www.") || [".com/", ".nz/", ".org/", ".net/", ".pe/", ".es/"].iter().any(|d| w.contains(d))
    })?;
    let w = w.trim_matches(|c: char| matches!(c, '(' | ')' | '«' | '»' | '"' | ',' | ';'));
    Some(if w.to_lowercase().starts_with("http") { w.to_string() } else { format!("https://{w}") })
}

fn color_motor(c: Option<[u8; 3]>) -> ColorRgba {
    match c {
        Some([r, g, b]) => ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        None => negro(),
    }
}

#[allow(clippy::too_many_arguments)]
fn un_texto(t: &str, x: f64, y: f64, tam: f64, familia: &str, negrita: bool, al: Alineacion, color: Option<[u8; 3]>) -> Elemento {
    let estilo = Estilo { color: color_motor(color), tam: tam as f32, opacidad: 1.0, familia: familia.to_string() };
    let medida = medir(t, tam as f32, familia, negrita);
    let mut el = elemento_texto(t, x as f32, y as f32, medida, tam as f32, false, &estilo);
    el.extras.negrita = negrita;
    el.extras.alineacion = Some(alineacion_motor(al));
    // Un enlace sigue siendo un enlace: el lienzo lo abre con Ctrl+clic.
    el.enlace = enlace_de(t);
    el
}

/// **Un parrafo o un encabezado** (con su marco, si estaba en un recuadro).
fn texto(e: &Entrada, titulo: bool) -> Vec<Value> {
    let z = e.zona;
    if z.renglones.is_empty() {
        return Vec::new();
    }
    let familia = if titulo { &e.opciones.familia_titulos } else { &e.opciones.familia };
    let negrita = titulo && (z.renglones.iter().any(|r| r.negrita) || z.renglones.iter().all(|r| r.alto() <= e.alto_letra * 1.3));
    let negrita = negrita || z.renglones.iter().all(|r| r.negrita);
    let esc = e.mapa.escala;
    let tam = tam_de_letra(&z.renglones, esc, familia, negrita);
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    // Que el parrafo no pase del alto que tenia en el papel: con la letra
    // elegida por el ancho, una cursiva estrecha salia mas alta y se montaba
    // en el parrafo de abajo y en la raya del recuadro (banco del 15-oct,
    // amor-a-sofia).
    let alto_papel = e.destino().h;
    let n = z.renglones.len() as f64;
    let tam = if n >= 2.0 && alto_papel > 0.0 { tam.min(alto_papel * 1.04 / (n * k)).max(tam * 0.7) } else { tam };
    let paso = {
        let v: Vec<f64> = z.renglones.windows(2).map(|w| (w[1].caja.cy() - w[0].caja.cy()) * esc).filter(|d| *d > 0.0).collect();
        mediana(v)
    };
    let color = super::seccion::color_mayoritario(z.renglones.iter().map(|r| (r.color, r.texto.chars().count())));
    // Renglones de varios enlaces: cada uno suelto, para que cada uno abra
    // el suyo.
    let enlaces = z.renglones.iter().filter(|r| enlace_de(&r.texto).is_some()).count();
    let suelto = enlaces > 1 || paso.is_some_and(|p| (p / (tam * k) - 1.0).abs() > 0.15);
    let mut f = e.fabrica();
    let mut piezas = Vec::new();
    if suelto {
        for r in &z.renglones {
            let d = e.mapa.caja(&r.caja);
            piezas.push(un_texto(&r.texto, d.x, d.cy() - tam * k / 2.0, tam, familia, negrita || r.negrita, Alineacion::Izquierda, r.color.or(color)));
        }
    } else {
        let todo = z.renglones.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>().join("\n");
        let primero = e.mapa.caja(&z.renglones[0].caja);
        let y = primero.cy() - tam * k / 2.0;
        let d = e.destino();
        let (w, _) = medir(&todo, tam as f32, familia, negrita);
        let x = match z.alineacion {
            Alineacion::Centro => d.cx() - w as f64 / 2.0,
            Alineacion::Derecha => d.x1() - w as f64,
            _ => z.renglones.iter().map(|r| e.mapa.caja(&r.caja).x).fold(f64::MAX, f64::min),
        };
        piezas.push(un_texto(&todo, x, y, tam, familia, negrita, z.alineacion, color));
    }
    f.de_motor(&piezas);
    // El marco de un recuadro.
    for r in &z.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * esc);
    }
    f.salida
}

/// **Cada renglon en su sitio**, con las rayas de la zona: lo fiel.
fn fiel(e: &Entrada) -> Vec<Value> {
    let z = e.zona;
    let familia = &e.opciones.familia;
    let esc = e.mapa.escala;
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    let mut piezas = Vec::new();
    for r in &z.renglones {
        let tam = tam_de_letra(std::slice::from_ref(r), esc, familia, r.negrita);
        let d = e.mapa.caja(&r.caja);
        piezas.push(un_texto(&r.texto, d.x, d.cy() - tam * k / 2.0, tam, familia, r.negrita, Alineacion::Izquierda, r.color));
    }
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    for r in &z.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * esc);
    }
    f.salida
}

fn recta(f: &mut Fabrica, a: (f64, f64), b: (f64, f64), grosor: f64) {
    f.trazo("line", &[a, b], NEGRO, None);
    if let Some(v) = f.salida.last_mut() {
        v["roughness"] = json!(0);
        v["strokeWidth"] = json!((grosor.max(1.0) * 2.0).round() / 2.0);
    }
}

fn linea(e: &Entrada) -> Vec<Value> {
    let mut f = e.fabrica();
    for r in &e.zona.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * e.mapa.escala);
    }
    f.salida
}

// ---------------------------------------------------------------------------
// Tabla

/// **La tabla dibujada** del motor, con la letra que la hace tan ancha como
/// la del papel. `None` si no hay celdas.
fn tabla(e: &Entrada) -> Option<Vec<Value>> {
    let t = e.zona.tabla.as_ref()?;
    if t.filas.is_empty() || t.columnas() == 0 {
        return None;
    }
    let familia = e.opciones.familia.clone();
    let d = e.destino();
    let medir_t = |s: &str, tam: f32| crate::lienzo::medir_con(s, tam, &familia);
    let cabecera = t.parece_cabecera();
    let color = super::seccion::color_mayoritario(e.zona.renglones.iter().map(|r| (r.color, r.texto.chars().count())));
    let hacer = |tam: f64| {
        let estilo = Estilo { color: color_motor(color), tam: tam as f32, opacidad: 1.0, familia: familia.clone() };
        pixpin_motor2d::tabla_dibujada::elementos_de_tabla(&t.filas, &estilo, Punto2::nuevo(d.x as f32, d.y as f32), &medir_t, cabecera)
    };
    let caja_de = |els: &[Elemento]| {
        els.iter()
            .map(Elemento::caja)
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            .map(|(x0, y0, x1, y1)| Caja::de_esquinas(x0 as f64, y0 as f64, x1 as f64, y1 as f64))
    };
    let mut tam = (e.alto_letra * e.mapa.escala).max(6.0);
    let mut els = hacer(tam);
    // Dos vueltas bastan: el aire de las celdas no escala con la letra.
    for _ in 0..2 {
        let c = caja_de(&els)?;
        if c.w <= 1.0 {
            break;
        }
        let k = (d.w / c.w).min(if c.h > 1.0 { d.h * 1.25 / c.h } else { 10.0 });
        let nuevo = (tam * k).clamp(6.0, 72.0);
        if (nuevo - tam).abs() < 0.5 {
            break;
        }
        tam = nuevo;
        els = hacer(tam);
    }
    if els.is_empty() {
        return None;
    }
    // Los enlaces de las celdas, abribles y de su color.
    for el in &mut els {
        if let pixpin_motor2d::Figura::Texto { texto, .. } = &el.figura
            && let Some(url) = enlace_de(texto)
        {
            if let Some(c) = e.zona.renglones.iter().find(|r| r.texto.contains(texto.as_str()) || texto.contains(r.texto.as_str())).and_then(|r| r.color) {
                el.trazo = color_motor(Some(c));
            }
            el.enlace = Some(url);
        }
    }
    let mut f = e.fabrica();
    f.de_motor(&els);
    Some(f.salida)
}

// ---------------------------------------------------------------------------
// Foto

/// Lo mas grande que se guarda una foto: dos veces su caja en el lienzo (se
/// ve nitida con zoom) y nunca mas de 2000 px de lado.
const LADO_FOTO: f64 = 2000.0;

/// Un id para el almacen de imagenes: distinto por hoja y zona, nunca cero,
/// por debajo de 2^48 (el `pc<hex>` cabe en doce cifras).
pub fn id_de_imagen(semilla: u64, zona: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ semilla;
    for b in (zona as u64).to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h & 0x0000_ffff_ffff_ffff).max(2)
}

/// **El recorte, como imagen del lienzo.**
fn foto(e: &Entrada) -> (Vec<Value>, Vec<ImagenNueva>) {
    match foto_de_caja(e, &e.zona.caja, 0, None) {
        Some((v, img)) => (vec![v], vec![img]),
        None => (Vec::new(), Vec::new()),
    }
}

/// **Un trozo de la hoja como imagen del lienzo** (la foto de la zona, o el
/// icono `k` de una celda), en su sitio. `grupo`: el de la zona, si ya hay.
fn foto_de_caja(e: &Entrada, caja: &Caja, k: usize, grupo: Option<Value>) -> Option<(Value, ImagenNueva)> {
    let (rgba, w, h) = e.pagina;
    let (rgba, w, h) = imagen::recortar(rgba, w, h, caja);
    if w == 0 || h == 0 {
        return None;
    }
    let d = e.mapa.caja(caja);
    let lado = (d.w.max(d.h) * 2.0).clamp(64.0, LADO_FOTO);
    let (rgba, w, h) = reducir(&rgba, w, h, lado as u32);
    let id = id_de_imagen(e.semilla, e.zona.id * 1000 + k);
    // Su propio id, que el de una fabrica nueva (`ia-<zona>-2`) es el mismo
    // que el de la primera pieza de la tabla que lleva este icono.
    let mut f = e.fabrica();
    let mut v = f.base("image", [d.x, d.y, d.w, d.h], "transparent", None);
    v["id"] = json!(format!("ia-{:x}-foto-{k}", e.semilla_de_zona()));
    v["fileId"] = json!(pixpin_motor2d::enlace::id_de_texto(id));
    v["status"] = json!("saved");
    v["scale"] = json!([1.0, 1.0]);
    v["roughness"] = json!(0);
    v["strokeWidth"] = json!(0);
    if let Some(g) = grupo {
        v["groupIds"] = g;
    }
    Some((v, ImagenNueva { id, rgba, ancho: w, alto: h }))
}

/// Reduce (promediando) si el lado mayor pasa de `lado`.
pub fn reducir(rgba: &[u8], w: u32, h: u32, lado: u32) -> (Vec<u8>, u32, u32) {
    let mayor = w.max(h);
    if mayor <= lado || lado == 0 {
        return (rgba.to_vec(), w, h);
    }
    let k = mayor as f64 / lado as f64;
    let (nw, nh) = (((w as f64 / k).round() as u32).max(1), ((h as f64 / k).round() as u32).max(1));
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let (sy0, sy1) = ((y as f64 * k) as u32, (((y + 1) as f64 * k) as u32).min(h).max((y as f64 * k) as u32 + 1));
        for x in 0..nw {
            let (sx0, sx1) = ((x as f64 * k) as u32, (((x + 1) as f64 * k) as u32).min(w).max((x as f64 * k) as u32 + 1));
            let mut s = [0u32; 4];
            let mut n = 0;
            for yy in sy0..sy1.min(h) {
                for xx in sx0..sx1.min(w) {
                    let i = ((yy * w + xx) * 4) as usize;
                    for c in 0..4 {
                        s[c] += rgba[i + c] as u32;
                    }
                    n += 1;
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (s[c] / n.max(1)) as u8;
            }
        }
    }
    (out, nw, nh)
}

// ---------------------------------------------------------------------------
// Formula

/// **La ecuacion compuesta** (`escribir_ecuacion`), encajada en su caja.
fn formula(e: &Entrada, latex: &[String]) -> Option<Vec<Value>> {
    let mut args = Map::new();
    args.insert("ecuacion".into(), json!(latex.join("\n")));
    let mut avisos = Vec::new();
    let mut ec = crate::ecuacion::validar(&args, &mut avisos).ok()?;
    let d = e.destino();
    ec.color = NEGRO.into();
    ec.tam = (e.alto_letra * e.mapa.escala * 1.1).clamp(8.0, 96.0);
    ec.donde = Some((d.x, d.y));
    let (_, (w, h)) = crate::ecuacion::piezas(&ec);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let k = (d.h / h as f64).min(d.w * 1.15 / w as f64).clamp(0.5, 2.0);
    ec.tam = (ec.tam * k).clamp(8.0, 120.0);
    let (_, (w, h)) = crate::ecuacion::piezas(&ec);
    // Centrada en la caja del papel.
    ec.donde = Some((d.cx() - w as f64 / 2.0, d.cy() - h as f64 / 2.0));
    let (piezas, _) = crate::ecuacion::piezas(&ec);
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    Some(f.salida)
}

// ---------------------------------------------------------------------------
// Calco y grafica

/// **El calco del recorte**, recolocado en la caja destino.
fn calco(e: &Entrada) -> Vec<Value> {
    let (rgba, w, h) = e.recorte();
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let p = crate::calcar::Parametros { detalle: crate::calcar::Detalle::Medio, ..Default::default() };
    let c = crate::calcar::calcar(&rgba, w, h, &p);
    calco_en_caja(&c, e.destino(), e.mapa.escala, e.semilla_de_zona())
}

/// **El enganche con el calcado**: sus trazos vienen en el lienzo de 800x600
/// (`lienzo = original * escala + desplazamiento`); aqui vuelven a pixeles
/// del recorte y de ahi a la caja destino.
pub fn calco_en_caja(c: &crate::calcar::Calcado, destino: Caja, escala_pagina: f64, semilla: u64) -> Vec<Value> {
    use crate::calcar::{Calcado, Trazo};
    if c.escala <= 0.0 {
        return Vec::new();
    }
    let a = |(x, y): (f64, f64)| {
        (destino.x + (x - c.desplazamiento.0) / c.escala * escala_pagina, destino.y + (y - c.desplazamiento.1) / c.escala * escala_pagina)
    };
    let todos = |v: &[(f64, f64)]| v.iter().map(|&p| a(p)).collect::<Vec<_>>();
    let trazos = c
        .trazos
        .iter()
        .map(|t| match t {
            Trazo::Mancha { contorno, huecos, color } => Trazo::Mancha {
                contorno: todos(contorno),
                huecos: huecos.iter().map(|h| todos(h)).collect(),
                color: color.clone(),
            },
            Trazo::Linea { puntos, cerrada, color } => Trazo::Linea { puntos: todos(puntos), cerrada: *cerrada, color: color.clone() },
            Trazo::Fondo { caja, color } => {
                let (x0, y0) = a((caja[0], caja[1]));
                let (x1, y1) = a((caja[0] + caja[2], caja[1] + caja[3]));
                Trazo::Fondo { caja: [x0, y0, x1 - x0, y1 - y0], color: color.clone() }
            }
        })
        .collect();
    let recolocado = Calcado { trazos, escala: 1.0, desplazamiento: (0.0, 0.0) };
    crate::lienzo::elementos_de_calcado(&recolocado, (0.0, 0.0), semilla)
}

/// **Una grafica de funciones** de verdad (`graficar_funcion`) en su caja.
fn grafica(e: &Entrada, args: &Map<String, Value>) -> Option<Vec<Value>> {
    let mut avisos = Vec::new();
    let p = crate::funciones::validar(args, &mut avisos).ok()?;
    let d = e.destino();
    let piezas = crate::funciones::piezas(&p, (d.x, d.y, d.w.max(120.0), d.h.max(90.0)));
    if piezas.is_empty() {
        return None;
    }
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    Some(f.salida)
}
