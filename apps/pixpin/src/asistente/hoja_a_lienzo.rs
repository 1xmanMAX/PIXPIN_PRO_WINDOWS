//! **`hoja_a_lienzo`, del lado de la app** (14-oct-2026): leer la hoja
//! (una foto con el OCR de Windows, o una pagina de PDF con su capa de texto
//! o su OCR), seccionarla y montarla (`pixpin_ia::hoja`), repartir el
//! trabajo y dejar cada zona en el lienzo **en cuanto esta hecha**.
//!
//! **Solo con la nube** (decision del usuario): con el modelo local no se
//! intenta nada y se contesta `hoja::SOLO_NUBE`.
//!
//! ## Latencia
//!
//! - Lo que no necesita modelo (texto, tablas, fotos, rayas, dibujos) se
//!   hace aqui en milisegundos y sale ya, en orden de lectura.
//! - Lo que si (formulas a LaTeX, zonas dudosas, graficas de funciones) va
//!   a la nube **antes** que lo local, en un pool de
//!   `Opciones::concurrencia` hilos (4: prudente con los 429; el motor ya
//!   reintenta respetando `retry-after`, y aqui un 429 se reintenta una vez
//!   mas tras una pausa). Cada peticion lleva SOLO el recorte de su zona y
//!   el motor va en modo ligero (sin pensar).
//! - El chat ve «3/9 zonas» mientras tanto (`Evento::Llamando`).
//!
//! ## Deshacer
//!
//! Cada zona entra en su paso (el lienzo puede estar abierto y el usuario
//! dibujando mientras llegan) y al acabar, si nadie toco el lienzo entre
//! medias, todos se funden en **un solo paso** (`al_lienzo::meter_de_hoja`,
//! `Escena::fundir_ultimos_pasos`): Ctrl+Z quita la hoja entera. Si el
//! usuario dibujo entre medias, quedan por zona: fundirlos se llevaria su
//! trazo con ellos.
//!
//! ## Comprobar
//!
//! Al final se pinta lo hecho (por el mismo camino que el PDF del lienzo) y
//! se compara zona a zona con el original (`hoja::comparar`); la que se
//! parece poco se rehace **una vez** por otro camino (renglones en su sitio,
//! o el recorte como imagen) y sustituye a la anterior en el lienzo.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_ia::hoja::montaje::{self, Entrada, Hecho, Leido, Via};
use pixpin_ia::hoja::{self, Caja, Mapa, Opciones, PalabraHoja, Seccionado, Tipo, comparar, imagen, vision};
use pixpin_ia::motor::Motor;

use super::al_lienzo::EnHoja;

#[cfg(test)]
mod banco;
#[cfg(test)]
mod banco_17oct;
pub(crate) mod plan;
#[cfg(test)]
pub(crate) mod banco_diagramas;
#[cfg(test)]
pub(crate) mod pruebas;

/// El ancho al que se pinta una pagina de PDF (~200 ppp en A4).
pub const ANCHO_PAGINA: u32 = 1700;
/// Una foto mas grande se reduce antes de leerla (el OCR no gana nada).
const LADO_MAXIMO: u32 = 2600;

/// La pagina, lista para seccionar.
pub struct Pagina {
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
    pub palabras: Vec<PalabraHoja>,
    /// Lo que tardo leerla (cargar y OCR).
    pub ms_leer: u128,
}

/// **Lee la hoja**: una imagen (OCR de Windows) o la pagina `pagina` (desde
/// 0) de un PDF (su capa de texto si la tiene; si no, OCR).
pub fn leer(ruta: &Path, pagina: Option<usize>) -> Result<Pagina, String> {
    let t = Instant::now();
    if super::ocr::es_imagen(ruta) {
        let mut img = pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
        let mayor = img.ancho.max(img.alto);
        if mayor > LADO_MAXIMO {
            let k = LADO_MAXIMO as f64 / mayor as f64;
            let (w, h) = (((img.ancho as f64 * k) as u32).max(1), ((img.alto as f64 * k) as u32).max(1));
            img = pixpin_codec::redimensionar(img, w, h).map_err(|e| format!("no se pudo reducir la imagen: {e:?}"))?;
        }
        let palabras = ocr(&img.pixeles, img.ancho, img.alto)?;
        return Ok(Pagina { rgba: img.pixeles, ancho: img.ancho, alto: img.alto, palabras, ms_leer: t.elapsed().as_millis() });
    }
    if !super::ocr::es_pdf(ruta) {
        return Err("solo se pasa al lienzo una foto (PNG, JPG...) o un PDF".into());
    }
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir el PDF: {e}"))?;
    let p = pagina.unwrap_or(0);
    if p as u32 >= doc.paginas() {
        return Err(format!("el PDF tiene {} paginas", doc.paginas()));
    }
    let img = doc.renderizar(p as u32, ANCHO_PAGINA).map_err(|e| e.to_string())?;
    // `medidas` da la hoja en DIP (1/96 de pulgada, la `Size` de Windows) y
    // la capa de texto va en puntos (1/72): medido con la tesis, sin pasar
    // de una a otra el texto quedaba en tres cuartos de la hoja.
    let ancho_pt = doc.medidas().get(p).map_or(0.0, |m| m.0 * 72.0 / 96.0);
    let trozos = pixpin_pdf::texto::trozos_de_fichero(ruta).ok().and_then(|t| t.get(p).cloned()).unwrap_or_default();
    let palabras = if trozos.len() >= 4 && ancho_pt > 0.0 {
        palabras_de_trozos(&trozos, img.ancho as f64 / ancho_pt as f64)
    } else {
        ocr(&img.pixeles, img.ancho, img.alto)?
    };
    Ok(Pagina { rgba: img.pixeles, ancho: img.ancho, alto: img.alto, palabras, ms_leer: t.elapsed().as_millis() })
}

/// Las palabras del OCR de Windows.
pub fn ocr(rgba: &[u8], ancho: u32, alto: u32) -> Result<Vec<PalabraHoja>, String> {
    if !pixpin_ocr::disponible() {
        return Err("el OCR de Windows no esta disponible (falta el idioma en Windows)".into());
    }
    let lineas = pixpin_ocr::reconocer(ancho, alto, rgba).map_err(|e| format!("el OCR fallo: {e}"))?;
    Ok(lineas
        .iter()
        .flat_map(|l| l.palabras.iter())
        .map(|p| PalabraHoja::nueva(&p.texto, p.caja.x as f64, p.caja.y as f64, p.caja.ancho as f64, p.caja.alto as f64))
        .collect())
}

/// Los trozos de la capa de texto del PDF (en puntos, `y` la linea base),
/// partidos en palabras (el ancho repartido por letras) y en pixeles.
pub fn palabras_de_trozos(trozos: &[pixpin_pdf::texto::TrozoDeTexto], esc: f64) -> Vec<PalabraHoja> {
    let mut v = Vec::new();
    for t in trozos {
        let total = t.texto.chars().count().max(1) as f64;
        let paso = (t.fin - t.x).max(0.0) as f64 / total;
        let (y, alto) = ((t.y - t.alto * 0.8) as f64, t.alto as f64);
        let mut i = 0usize;
        for trozo in t.texto.split(' ') {
            let n = trozo.chars().count();
            if !trozo.trim().is_empty() {
                let x = t.x as f64 + i as f64 * paso;
                let mut p = PalabraHoja::nueva(trozo, x * esc, y * esc, n as f64 * paso * esc, alto * esc);
                p.negrita = t.negrita;
                v.push(p);
            }
            i += n + 1;
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Adonde va cada zona

/// **Quien recibe lo hecho**: la cola del lienzo (chat o lienzo abierto),
/// el lienzo del asistente sin ventana (Flow), o una escena (pruebas).
pub trait Destino {
    /// Una zona hecha (`None`: solo cerrar la hoja, con `en.ultima`).
    fn entregar(&mut self, h: Option<&Hecho>, en: EnHoja) -> Result<(), String>;
    /// Espera a que todo haya entrado; el primer error, si lo hubo.
    fn esperar(&mut self, _parar: &AtomicBool) -> Result<(), String> {
        Ok(())
    }
}

/// Los elementos y las imagenes de una zona hecha, como van al lienzo.
pub fn documento_de(h: Option<&Hecho>) -> (String, Vec<(u64, pixpin_codec::ImagenRgba)>) {
    match h {
        // Vacio tambien va (como documento sin nada): rehacer con nada es
        // quitar la zona (`al_lienzo::meter_de_hoja`).
        Some(h) => (
            pixpin_ia::lienzo::documento(h.elementos.clone()),
            h.imagenes
                .iter()
                .map(|i| (i.id, pixpin_codec::ImagenRgba { ancho: i.ancho, alto: i.alto, pixeles: i.rgba.clone() }))
                .collect(),
        ),
        _ => (String::new(), Vec::new()),
    }
}

/// **Por la cola de `al_lienzo`**: el lienzo abierto o, si no hay, el chat.
/// No se espera a cada zona (la ventana las mete de una vuelta); al final,
/// [`Destino::esperar`] recoge las respuestas.
pub struct PorLaCola {
    pub proyecto: String,
    pub destino: Option<String>,
    pub espera: Duration,
    respuestas: Vec<Receiver<Result<super::al_lienzo::Respuesta, String>>>,
    /// La imagen del lienzo de la que sale la hoja: va a su lado.
    pub junto_a: Option<(f32, f32, f32, f32)>,
}

impl PorLaCola {
    pub fn nuevo(proyecto: String, destino: Option<String>, espera: Duration) -> PorLaCola {
        PorLaCola { proyecto, destino, espera, respuestas: Vec::new(), junto_a: None }
    }

    /// Junto a la imagen del lienzo de la que sale (su caja).
    pub fn junto_a(mut self, c: Option<(f32, f32, f32, f32)>) -> PorLaCola {
        self.junto_a = c;
        self
    }
}

impl Destino for PorLaCola {
    fn entregar(&mut self, h: Option<&Hecho>, mut en: EnHoja) -> Result<(), String> {
        en.junto_a = en.junto_a.or(self.junto_a);
        let (documento, imagenes) = documento_de(h);
        let (tx, rx) = channel();
        super::al_lienzo::pedir(
            super::al_lienzo::Pedido::nuevo(self.proyecto.clone(), self.destino.clone(), documento, tx).en_hoja(en).con_imagenes(imagenes),
        );
        self.respuestas.push(rx);
        Ok(())
    }

    fn esperar(&mut self, parar: &AtomicBool) -> Result<(), String> {
        let hasta = Instant::now() + self.espera;
        let mut primero: Option<String> = None;
        for rx in self.respuestas.drain(..) {
            loop {
                if parar.load(Ordering::Relaxed) {
                    return Err("el usuario lo paro".into());
                }
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(_)) => break,
                    Ok(Err(e)) => {
                        primero.get_or_insert(e);
                        break;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        primero.get_or_insert("la ventana del chat no contesto".into());
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) if Instant::now() >= hasta => {
                        return Err("la ventana del chat no contesto a tiempo".into());
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        }
        primero.map_or(Ok(()), Err)
    }
}

// ---------------------------------------------------------------------------
// El trabajo

/// Lo que se le pregunta al modelo de una zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pregunta {
    Latex,
    Tipo,
    Grafica,
    Rotulos,
    /// Un bloque de texto, en texto rico.
    Rico,
    /// Los renglones de una zona de texto, corregidos (cada uno en su sitio).
    Renglones,
    /// Una tabla: la tabla del lienzo con su aspecto (`tabla_de_fichero`).
    Tabla,
}

/// Si el LaTeX de una formula ya dice lo que dice `texto` (otra zona que
/// la pisa): casi todas sus letras y cifras estan en el. «x - y = 1» dentro
/// de `\begin{cases} 2x+3y=7 \\ x-y=1 \end{cases}`, si; «a)» no.
pub(crate) fn ya_lo_dice(latex: &str, texto: &str) -> bool {
    // Sin los mandatos (`\begin`, `\frac`...), que no son letras del papel.
    let mut limpio = String::new();
    // Y sin el nombre del entorno (`{cases}` no son letras del papel).
    let mut sin_entornos = latex.to_string();
    for m in ["\\begin{", "\\end{"] {
        while let Some(i) = sin_entornos.find(m) {
            let fin = sin_entornos[i..].find('}').map_or(sin_entornos.len(), |j| i + j + 1);
            sin_entornos.replace_range(i..fin, " ");
        }
    }
    let mut c = sin_entornos.chars().peekable();
    while let Some(ch) = c.next() {
        if ch == '\\' {
            while c.peek().is_some_and(|d| d.is_ascii_alphabetic()) {
                c.next();
            }
        } else {
            limpio.push(ch);
        }
    }
    let mut hay: HashMap<char, usize> = HashMap::new();
    for ch in limpio.chars().filter(|c| c.is_alphanumeric()) {
        *hay.entry(ch.to_ascii_lowercase()).or_default() += 1;
    }
    let suyas: Vec<char> = texto.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
    if suyas.is_empty() {
        return true;
    }
    let mut dentro = 0;
    for ch in &suyas {
        if let Some(n) = hay.get_mut(ch).filter(|n| **n > 0) {
            *n -= 1;
            dentro += 1;
        }
    }
    dentro as f64 >= suyas.len() as f64 * 0.8
}

/// Que pregunta (si alguna) lleva una zona.
fn pregunta(z: &hoja::Zona, o: &Opciones, alto_letra: f64) -> Option<Pregunta> {
    match z.tipo {
        // Un simbolo suelto (el adorno de un separador): como imagen, sin
        // preguntar (el modelo no sabria que decir de tan poco).
        Tipo::Dudosa if z.caja.w < alto_letra * 3.0 && z.caja.h < alto_letra * 3.0 => None,
        Tipo::Formula => Some(Pregunta::Latex),
        // Una lamina (17-oct): el texto sale ya con el OCR, cada renglon en su
        // sitio, y la nube lo corrige (tildes, °, θ, ∢, formulas en linea).
        Tipo::Parrafo | Tipo::Encabezado if o.rico && !z.renglones.is_empty() => Some(Pregunta::Renglones),
        // Una tabla (la dijo el planificador, o el seccionado): su especialista.
        Tipo::Tabla if o.rico => Some(Pregunta::Tabla),
        Tipo::Dudosa => Some(Pregunta::Tipo),
        Tipo::Grafica => Some(Pregunta::Grafica),
        // Un dibujo de tinta (sin fotografia) puede ser una formula que el
        // seccionado no supo leer (las llaves de un sistema escaneado): se
        // pregunta siempre, y el calco queda para cuando de verdad es dibujo
        // (banco del 15-oct: los sistemas salian como calco negro grueso).
        Tipo::Dibujo if z.caja.w >= alto_letra * 1.5 || z.caja.h >= alto_letra * 1.5 => Some(Pregunta::Tipo),
        Tipo::Foto | Tipo::Dibujo if z.confianza < o.duda => Some(Pregunta::Tipo),
        _ => None,
    }
}

/// El recorte de `c` a su tamano, sin aire ni reducir (una tabla: su esquina
/// es la de la zona).
fn png_entero(p: &Pagina, c: &Caja) -> Result<Vec<u8>, String> {
    let (rgba, w, h) = imagen::recortar(&p.rgba, p.ancho, p.alto, c);
    pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: rgba }).map_err(|e| format!("{e:?}"))
}

/// Lo mas grande que se manda un recorte al modelo.
const LADO_AL_MODELO: u32 = 1024;

fn png_del_recorte(p: &Pagina, c: &Caja) -> Result<Vec<u8>, String> {
    // Un poco de aire alrededor: una formula pegada al borde se lee peor.
    let (rgba, w, h) = imagen::recortar(&p.rgba, p.ancho, p.alto, &c.crecer(6.0));
    let (rgba, w, h) = montaje::reducir(&rgba, w, h, LADO_AL_MODELO);
    pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: rgba }).map_err(|e| format!("{e:?}"))
}

fn preguntar(motor: &dyn Motor, png: Vec<u8>, q: Pregunta, pista: &str) -> Result<Option<Leido>, String> {
    match q {
        Pregunta::Latex => vision::pedir_latex(motor, png).map(Some),
        Pregunta::Tipo => vision::pedir_tipo(motor, png).map(Some),
        Pregunta::Grafica => vision::pedir_grafica(motor, png),
        Pregunta::Rotulos => vision::pedir_rotulos(motor, png).map(Some),
        Pregunta::Rico => vision::pedir_rico(motor, png, pista).map(Some),
        Pregunta::Renglones => {
            // (El tema de la hoja, del planificador, delante y separado por \u{1f}.)
            let (tema, renglones) = pista.split_once('\u{1f}').unwrap_or(("", pista));
            vision::pedir_renglones(motor, png, &renglones.split('\n').map(str::to_string).collect::<Vec<_>>(), tema).map(Some)
        }
        Pregunta::Tabla => tabla_del_recorte(motor, png, pista.parse().unwrap_or(1.0)).map(Some),
    }
}

/// **El especialista de tablas** (18-oct): el recorte de la zona a un
/// fichero y la tabla de foto al lienzo de siempre (`tabla_de_fichero`: celdas
/// del OCR corregidas por la nube, anchos, altos, fondos y borde medidos), a
/// `escala` del lienzo. La tabla de las capturas salia como texto suelto y un
/// pantallazo debajo.
fn tabla_del_recorte(motor: &dyn Motor, png: Vec<u8>, escala: f32) -> Result<Leido, String> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ruta = std::env::temp_dir().join(format!("pixpin-tabla-{}-{}.png", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::write(&ruta, &png).map_err(|e| e.to_string())?;
    let r = super::documentos_acciones::tabla_de_fichero(&ruta, "tabla", None, None, Some(escala), Some(motor));
    let _ = std::fs::remove_file(&ruta);
    let (els, _, origen) = r?;
    Ok(Leido::Elementos(pixpin_motor2d::excalidraw::elementos_a_json(&els), origen))
}

/// Lo que paso, para el modelo, el chat y el banco.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // los tiempos y parecidos los leen el registro, las pruebas y el banco
pub struct Informe {
    pub seccionado: Seccionado,
    /// Lo que quedo de cada zona (tras rehacer).
    pub hechos: Vec<Hecho>,
    /// El parecido de cada zona con el original (0-1), si se comprobo.
    pub parecidos: Vec<(usize, f64)>,
    pub rehechas: usize,
    pub ms_seccionar: u128,
    /// Desde que empezo hasta que la primera zona salio hacia el lienzo.
    pub ms_primera: Option<u128>,
    pub ms_total: u128,
    pub ms_comprobar: u128,
    /// Cuantas peticiones al modelo y las que fallaron (con su motivo).
    pub peticiones: usize,
    pub fallos: Vec<String>,
    /// Cuantos trabajadores de la nube se lanzaron y cuantas peticiones
    /// llegaron a estar en vuelo a la vez (lo medido, no lo pedido).
    pub trabajadores: usize,
    pub a_la_vez: usize,
}

/// **Pasa la hoja al lienzo.** `vision`: el modelo de la nube (ligero); sin
/// el, lo que no necesita modelo se hace igual y lo que si sale como
/// imagen. `semilla`: distinta en cada llamada. `fiel`: cada renglon en su
/// sitio exacto desde el principio.
#[allow(clippy::too_many_arguments)]
pub fn pasar(
    p: &Pagina,
    vision: Option<Arc<dyn Motor>>,
    o: &Opciones,
    fiel: bool,
    semilla: u64,
    parar: &AtomicBool,
    destino: &mut dyn Destino,
    progreso: &mut dyn FnMut(usize, usize),
) -> Result<Informe, String> {
    let t0 = Instant::now();
    // Por recuadros (17-oct): cada tarjeta o caja se secciona aparte, y lo
    // que es adorno (la marca de agua) no entra. Una lamina: cada dibujo, una
    // zona (con sus flechas y su texto de dentro).
    let (s, limpia) = hoja::paneles::seccionar_limpia(&p.rgba, p.ancho, p.alto, p.palabras.clone(), o.rico);
    // (Sin la marca de agua de las tarjetas: las zonas se montan de la hoja limpia.)
    let limpia_p;
    let p: &Pagina = match limpia {
        Some(rgba) => {
            limpia_p = Pagina { rgba, ancho: p.ancho, alto: p.alto, palabras: p.palabras.clone(), ms_leer: p.ms_leer };
            &limpia_p
        }
        None => p,
    };
    // **El planificador** (18-oct): con la nube y en una lamina, un agente
    // mira la hoja entera con sus zonas numeradas y dice de que trata, que es
    // cada una, cuales van juntas, que son contenedores y que se ignora
    // (`plan`). Sin respuesta, se sigue con lo de siempre.
    let mut tema = String::new();
    let s = match &vision {
        Some(m) if o.rico && std::env::var_os("PIXPIN_SIN_PLAN").is_none() => {
            let tp = Instant::now();
            match plan::pedir(m.as_ref(), p, &s) {
                Ok(pl) => {
                    tema = plan::contexto(&pl);
                    tracing::info!(tema = %pl.tema, zonas = s.zonas.len(), ms = tp.elapsed().as_millis() as u64, "hoja al lienzo: plan");
                    plan::aplicar(&pl, &s, p)
                }
                Err(e) => {
                    tracing::warn!(%e, "hoja al lienzo: sin plan");
                    s
                }
            }
        }
        _ => s,
    };
    let tema = tema;
    let ms_seccionar = t0.elapsed().as_millis();
    if s.zonas.is_empty() {
        return Err("la hoja esta en blanco: no hay nada que pasar al lienzo".into());
    }
    let mapa = Mapa::para(&s, o);
    let (hw, hh) = mapa.hoja(&s);
    let tam = (hw as f32, hh as f32);
    let sesion = semilla;
    let en = |zona: usize, rehacer: bool, ultima: bool| EnHoja { sesion, zona, rehacer, ultima, tam, junto_a: None };
    let lector = lector_de_letras();
    // El texto de cada zona: para las demas (un diagrama no lo vectoriza).
    let ajenas: Vec<Caja> = s.zonas.iter().flat_map(|z| z.renglones.iter().flat_map(|r| r.palabras.iter().map(|p| p.caja))).collect();
    // Y todo su texto: las pistas de la geometria exacta («AB es diametro»).
    let mut pistas: Vec<String> = s.zonas.iter().flat_map(|z| z.renglones.iter().map(|r| r.texto.clone())).collect();
    if !tema.is_empty() {
        pistas.push(tema.clone());
    }
    let pistas = pistas;
    let entrada = |id: usize| Entrada { zona: &s.zonas[id], mapa, opciones: o, pagina: (&p.rgba, p.ancho, p.alto), alto_letra: s.alto_letra, semilla, lector: Some(&lector), ajenas: &ajenas, pistas: &pistas };
    // (En una lamina, el texto con cada renglon en su sitio: rehecho como
    // parrafo se corria hacia abajo y pisaba lo de debajo.)
    let via_base = |z: &hoja::Zona| if (fiel && matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Tabla)) || (o.rico && matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado)) { Via::Fiel } else { Via::Editable };
    // **Los bloques de texto rico** (laminas con la nube): sus zonas no
    // preguntan por su cuenta; el bloque entero va a la nube de una vez.
    // (17-oct: ya no se piden por bloques: el bloque recolocado movia el
    // texto y la nube juntaba renglones ajenos. Cada zona de texto pide sus
    // renglones corregidos, `Pregunta::Renglones`.)
    let bloques: Vec<(usize, Vec<usize>, hoja::Zona)> = Vec::new();
    let miembros: std::collections::HashSet<usize> = bloques.iter().flat_map(|b| b.1.iter().copied()).collect();
    let es_bloque = |id: usize| bloques.iter().any(|b| b.0 == id);
    let pregunta_de = |z: &hoja::Zona| if miembros.contains(&z.id) { None } else { pregunta(z, o, s.alto_letra) };
    let pisa = |a: &Caja, b: &Caja| {
        let c = a.comun(b);
        c > 0.0 && c >= 0.4 * a.area().min(b.area())
    };
    // 0. **Lo primero que se ve, ya** (latencia, 15-oct): el texto, las
    //    tablas, las fotos y las rayas que no pisan nada que pueda esperar
    //    al modelo (una formula, un dibujo que quiza es diagrama) salen
    //    antes de mirar los dibujos, en milisegundos.
    let posibles: Vec<usize> = s
        .zonas
        .iter()
        .filter(|z| matches!(z.tipo, Tipo::Dibujo | Tipo::Grafica) || (vision.is_some() && pregunta_de(z).is_some_and(|q| q != Pregunta::Renglones)))
        .map(|z| z.id)
        .collect();
    let mut tempranas: Vec<Hecho> = Vec::new();
    let mut ms_temprana = None;
    for z in &s.zonas {
        if parar.load(Ordering::Relaxed) {
            break;
        }
        // (Un recuadro va siempre ya: es el fondo de lo suyo.)
        let suelta = z.tipo == Tipo::Recuadro
            || !posibles.contains(&z.id)
            && matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Linea | Tipo::Tabla | Tipo::Foto | Tipo::Recuadro)
            && !posibles.iter().any(|&e| pisa(&z.caja, &s.zonas[e].caja));
        if !suelta {
            continue;
        }
        let h = montaje::hacer(&entrada(z.id), None, via_base(z));
        destino.entregar(Some(&h), en(z.id, false, false))?;
        ms_temprana.get_or_insert(t0.elapsed().as_millis());
        progreso(tempranas.len() + 1, s.zonas.len());
        tempranas.push(h);
    }
    // Los dibujos que son diagramas (figuras exactas): salen ya, sin
    // esperar; a la nube solo se le preguntan sus rotulos, y al llegar se
    // rehacen con ellos. Se miran a la vez (un hilo por dibujo).
    // (Hechos ya, con sus letras leidas: al entregarlos no se rehacen.)
    let precalculados: std::cell::RefCell<HashMap<usize, Hecho>> = std::cell::RefCell::new({
        let candidatos: Vec<usize> = s.zonas.iter().filter(|z| matches!(z.tipo, Tipo::Dibujo | Tipo::Grafica)).map(|z| z.id).collect();
        std::thread::scope(|sc| {
            let hilos: Vec<_> = candidatos
                .iter()
                .map(|&id| {
                    (
                        id,
                        sc.spawn(move || {
                            // El OCR de las letras sueltas es WinRT: su COM.
                            let _com = pixpin_shell::ComDelHilo::iniciar();
                            montaje::hacer(&entrada(id), None, Via::Editable)
                        }),
                    )
                })
                .collect();
            hilos.into_iter().filter_map(|(id, h)| h.join().ok().filter(|h| h.tipo == Tipo::Diagrama).map(|h| (id, h))).collect()
        })
    });
    let diagramas: std::collections::HashSet<usize> = precalculados.borrow().keys().copied().collect();
    let mut preguntas: Vec<(usize, Pregunta)> = match &vision {
        Some(_) => s
            .zonas
            .iter()
            .filter_map(|z| if diagramas.contains(&z.id) { Some((z.id, Pregunta::Rotulos)) } else { pregunta_de(z).map(|q| (z.id, q)) })
            .collect(),
        None => Vec::new(),
    };
    // Y cada bloque de texto, con la clave de su primera zona.
    preguntas.extend(bloques.iter().map(|b| (b.0, Pregunta::Rico)));
    let total = s.zonas.len();
    let mut hechos: Vec<Option<Hecho>> = vec![None; total];
    for h in tempranas {
        let id = h.zona;
        hechos[id] = Some(h);
    }
    let mut leidos: HashMap<usize, Leido> = HashMap::new();
    let mut ms_primera = ms_temprana;
    let mut fallos = Vec::new();
    let mut error_entrega: Option<String> = None;
    let mut n_hechas = hechos.iter().flatten().count();
    let (tx, rx) = channel::<(usize, Result<Option<Leido>, String>)>();
    // 1. La nube primero: es lo que mas tarda. Hilos sueltos (no de un
    //    `scope`): si la red se cuelga, el pool no deja colgada la hoja; al
    //    pasar el plazo lo que falte sale como imagen.
    // **Un trabajador por zona, a la vez** (15-oct, el usuario: «que divida
    // la lamina y use varios agentes»): cada uno con SOLO su recorte y su
    // tarea (los rotulos y hechos de una figura, el LaTeX de una formula, que
    // es una dudosa). Cuantos a la vez, segun el motor: Claude 8, DeepSeek 3.
    let en_vuelo = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_en_vuelo = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut trabajadores = 0usize;
    if let Some(motor) = &vision
        && !preguntas.is_empty()
    {
        let trabajos: VecDeque<(usize, Pregunta, Result<Vec<u8>, String>, String)> =
            preguntas.iter().map(|&(id, q)| {
                let bloque = if q == Pregunta::Rico { bloques.iter().find(|b| b.0 == id) } else { None };
                let caja = bloque.map_or(s.zonas[id].caja, |b| b.2.caja);
                let pista = match q {
                    Pregunta::Renglones => format!("{tema}\u{1f}{}", s.zonas[id].renglones.iter().map(|r| r.texto.replace('\n', " ")).collect::<Vec<_>>().join("\n")),
                    Pregunta::Tabla => format!("{}", mapa.escala),
                    _ => bloque.map(|b| pista_de_colores(&b.2)).unwrap_or_default(),
                };
                // (La tabla, a su tamano y sin aire: su esquina es la de la zona.)
                let png = if q == Pregunta::Tabla { png_entero(p, &caja) } else { png_del_recorte(p, &caja) };
                (id, q, png, pista)

            }).collect();
        let cola = Arc::new(Mutex::new(trabajos));
        let a_la_vez = if motor.a_la_vez() > 1 { motor.a_la_vez() } else { o.concurrencia };
        trabajadores = a_la_vez.clamp(1, 8).min(preguntas.len());
        for _ in 0..trabajadores {
            let (cola, tx, motor) = (cola.clone(), tx.clone(), motor.clone());
            let (en_vuelo, max_en_vuelo) = (en_vuelo.clone(), max_en_vuelo.clone());
            let lanzado = std::thread::Builder::new().name("hoja-al-lienzo-nube".into()).spawn(move || {
                // (El especialista de tablas lee con el OCR de Windows: su COM.)
                let _com = pixpin_shell::ComDelHilo::iniciar();
                loop {
                    let Some((id, q, png, pista)) = cola.lock().ok().and_then(|mut c| c.pop_front()) else { break };
                    let ahora = en_vuelo.fetch_add(1, Ordering::SeqCst) + 1;
                    max_en_vuelo.fetch_max(ahora, Ordering::SeqCst);
                    let r = png.and_then(|png| match preguntar(motor.as_ref(), png.clone(), q, &pista) {
                        // Un 429 o un 529 que el motor ya reintento: una vez
                        // mas, con calma.
                        Err(e) if e.contains("Demasiadas peticiones") || e.contains("saturado") => {
                            std::thread::sleep(Duration::from_secs(3));
                            preguntar(motor.as_ref(), png, q, &pista)
                        }
                        r => r,
                    });
                    en_vuelo.fetch_sub(1, Ordering::SeqCst);
                    if tx.send((id, r)).is_err() {
                        break;
                    }
                }
            });
            if lanzado.is_err() {
                tracing::warn!("hoja al lienzo: no se pudo lanzar un hilo para la nube");
            }
        }
    }
    drop(tx);
    // **Zonas que se pisan** (banco del 15-oct, hojas escaneadas): el OCR
    // saca un renglon suelto («x - y = 1») dentro de un sistema que es
    // otra zona, o un trozo del titulo sale tambien como recorte. Los
    // textos que pisan una zona que espera al modelo se aplazan hasta su
    // respuesta: si esa zona sale editable (formula, tabla, grafica) los
    // lleva dentro y el texto sobra; si sale como imagen o calco y los
    // textos la cubren, sobra la imagen.
    // (Los renglones a corregir ya salieron con el OCR: no esperan.)
    let corregir: std::collections::HashSet<usize> = preguntas.iter().filter(|(_, q)| *q == Pregunta::Renglones).map(|(id, _)| *id).collect();
    let esperan: Vec<usize> = preguntas.iter().map(|(id, _)| *id).filter(|id| !diagramas.contains(id) && !es_bloque(*id) && !corregir.contains(id)).collect();
    let mut rehechos_diagrama: Vec<Hecho> = Vec::new();
    let aplazadas: Vec<usize> = s
        .zonas
        .iter()
        // (Y las rayas: la de una fraccion leida aparte salia doble, gruesa.)
        .filter(|z| matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Linea) && !esperan.contains(&z.id))
        .filter(|z| esperan.iter().any(|&e| pisa(&z.caja, &s.zonas[e].caja)))
        .map(|z| z.id)
        .collect();
    // Y las que esperan al modelo pero caen dentro de otra mas grande que
    // tambien espera (un renglon del sistema leido aparte): se retienen
    // hasta que conteste la grande.
    let contenidas: Vec<usize> = esperan
        .iter()
        .copied()
        .filter(|&a| {
            let ca = &s.zonas[a].caja;
            esperan.iter().any(|&b| b != a && s.zonas[b].caja.area() > ca.area() && ca.dentro_de(&s.zonas[b].caja) >= 0.6)
        })
        .collect();
    let retenidas: std::cell::RefCell<HashMap<usize, Option<Leido>>> = Default::default();
    // Los renglones corregidos que llegan antes de que su zona salga (un texto
    // aplazado): salen ya corregidos cuando toque, no dos veces.
    let mut corregidos: HashMap<usize, Leido> = HashMap::new();
    let mut sobran: std::collections::HashSet<usize> = Default::default();
    // Las zonas que ya van dentro de un bloque de texto rico: no se vuelven a poner.
    let absorbidas: std::cell::RefCell<std::collections::HashSet<usize>> = Default::default();
    let mut entregar = |id: usize, leido: Option<Leido>, destino: &mut dyn Destino, hechos: &mut Vec<Option<Hecho>>| {
        let z = &s.zonas[id];
        // Una raya suelta que cae dentro de un diagrama es de el (el lado
        // rojo OA de la figura salia aparte, marron y gorda, encima): ya va
        // con sus figuras exactas.
        let del_diagrama = z.tipo == Tipo::Linea && diagramas.iter().any(|&d| z.caja.dentro_de(&s.zonas[d].caja) >= 0.8);
        if sobran.contains(&id) || del_diagrama || absorbidas.borrow().contains(&id) {
            n_hechas += 1;
            progreso(n_hechas, total);
            return;
        }
        let precalculado = if leido.is_none() { precalculados.borrow_mut().remove(&id) } else { None };
        let h = precalculado.unwrap_or_else(|| montaje::hacer(&entrada(id), leido.as_ref(), via_base(z)));
        let latex = match &leido {
            Some(Leido::Latex(l)) => Some(l.join(" ")),
            _ => None,
        };
        if let Some(l) = leido {
            leidos.insert(id, l);
        }
        let textos: Vec<usize> = aplazadas
            .iter()
            .chain(&contenidas)
            .copied()
            .filter(|&t| t != id && hechos[t].is_none() && pisa(&s.zonas[t].caja, &z.caja))
            .collect();
        if !textos.is_empty() {
            let como_imagen = h.via == Via::Imagen || h.nota.contains("imagen") || h.nota.contains("calc");
            let editable = !como_imagen && matches!(h.tipo, Tipo::Formula | Tipo::Tabla | Tipo::Grafica);
            let cubierto: f64 = textos.iter().map(|&t| s.zonas[t].caja.comun(&z.caja)).sum::<f64>() / z.caja.area().max(1.0);
            if editable {
                // Solo sobra lo que la formula ya dice: «a)» delante de un
                // sistema no esta en su LaTeX y se queda.
                for t in textos {
                    let suyo = match retenidas.borrow().get(&t) {
                        Some(Some(Leido::Latex(l))) => l.join(" "),
                        _ => s.zonas[t].renglones.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>().join(" "),
                    };
                    if latex.as_deref().is_none_or(|l| ya_lo_dice(l, &suyo)) {
                        sobran.insert(t);
                    }
                }
            } else if como_imagen && cubierto >= 0.5 {
                tracing::info!(zona = id, "hoja al lienzo: imagen tapada por texto, sobra");
                n_hechas += 1;
                progreso(n_hechas, total);
                return;
            }
        }
        if let Err(e) = destino.entregar(Some(&h), en(id, false, false)) {
            error_entrega.get_or_insert(e);
        }
        ms_primera.get_or_insert(t0.elapsed().as_millis());
        n_hechas += 1;
        progreso(n_hechas, total);
        hechos[id] = Some(h);
    };
    // 2. Lo que no necesita modelo, ya, en orden de lectura (menos los
    //    textos aplazados).
    let ya: Vec<usize> = (0..total).filter(|&i| hechos[i].is_some()).collect();
    for z in s.zonas.iter().filter(|z| !esperan.contains(&z.id) && !aplazadas.contains(&z.id) && !ya.contains(&z.id)) {
        if parar.load(Ordering::Relaxed) {
            break;
        }
        entregar(z.id, None, destino, &mut hechos);
    }
    // 3. Lo del modelo, segun llega, hasta el plazo.
    let plazo = Instant::now() + o.plazo_nube;
    // (Con los diagramas: sus rotulos llegan despues de que ya se vean.)
    let mut pendientes: Vec<usize> = preguntas.iter().map(|(id, _)| *id).collect();
    while !pendientes.is_empty() && !parar.load(Ordering::Relaxed) {
        let queda = plazo.saturating_duration_since(Instant::now());
        match rx.recv_timeout(queda.min(Duration::from_millis(200))) {
            Ok((id, r)) => {
                pendientes.retain(|x| *x != id);
                let leido = match r {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::warn!(%e, zona = id, "hoja al lienzo: el modelo no pudo con una zona");
                        fallos.push(format!("zona {id}: {e}"));
                        None
                    }
                };
                if let Some(b) = bloques.iter().find(|b| b.0 == id) {
                    // El bloque en texto rico: la primera zona lo lleva (un
                    // solo texto) y las demas se vacian.
                    if let Some(l) = leido.filter(|l| matches!(l, Leido::Rico(_))) {
                        let eb = Entrada { zona: &b.2, mapa, opciones: o, pagina: (&p.rgba, p.ancho, p.alto), alto_letra: s.alto_letra, semilla, lector: Some(&lector), ajenas: &ajenas, pistas: &pistas };
                        let h = montaje::hacer(&eb, Some(&l), Via::Editable);
                        if !h.elementos.is_empty() && destino.entregar(Some(&h), en(id, true, false)).is_ok() {
                            rehechos_diagrama.push(h);
                            for &m in &b.1[1..] {
                                absorbidas.borrow_mut().insert(m);
                                let vacio = Hecho { zona: m, tipo: Tipo::Parrafo, via: Via::Editable, elementos: Vec::new(), imagenes: Vec::new(), nota: format!("en el bloque de texto rico de la zona {id}") };
                                if destino.entregar(Some(&vacio), en(m, true, false)).is_ok() {
                                    rehechos_diagrama.push(vacio);
                                }
                            }
                        }
                    }
                } else if diagramas.contains(&id) {
                    // Los rotulos de un diagrama que ya esta en el lienzo: se
                    // rehace con ellos (la geometria es la misma).
                    match leido {
                        Some(l) => {
                            let h = montaje::hacer(&entrada(id), Some(&l), Via::Editable);
                            if h.tipo == Tipo::Diagrama && destino.entregar(Some(&h), en(id, true, false)).is_ok() {
                                rehechos_diagrama.push(h);
                            }
                        }
                        None => {}
                    }
                } else if corregir.contains(&id) {
                    // Sus renglones corregidos, en el sitio de los del OCR.
                    if hechos[id].is_none() {
                        if let Some(l) = leido {
                            corregidos.insert(id, l);
                        }
                    } else if let Some(l) = leido {
                        let h = montaje::hacer(&entrada(id), Some(&l), Via::Fiel);
                        if !h.elementos.is_empty() && destino.entregar(Some(&h), en(id, true, false)).is_ok() {
                            rehechos_diagrama.push(h);
                        }
                    }
                } else if contenidas.contains(&id) {
                    retenidas.borrow_mut().insert(id, leido);
                } else {
                    entregar(id, leido, destino, &mut hechos);
                }
            }
            Err(RecvTimeoutError::Timeout) if Instant::now() < plazo => {}
            Err(_) => break,
        }
    }
    // Lo que no llego a tiempo (la red colgada): sin modelo.
    for id in pendientes.into_iter().filter(|id| !diagramas.contains(id) && !es_bloque(*id) && !corregir.contains(id)) {
        if parar.load(Ordering::Relaxed) {
            break;
        }
        fallos.push(format!("zona {id}: el modelo no contesto a tiempo"));
        entregar(id, None, destino, &mut hechos);
    }
    // Las retenidas (ya contesto la que las contiene) y los textos
    // aplazados que no quedaron dentro de otra zona.
    let mut quedan: Vec<(usize, Option<Leido>)> = retenidas.borrow_mut().drain().collect();
    quedan.sort_by_key(|(id, _)| *id);
    for (id, leido) in quedan {
        if parar.load(Ordering::Relaxed) {
            break;
        }
        entregar(id, leido, destino, &mut hechos);
    }
    for &id in &aplazadas {
        if parar.load(Ordering::Relaxed) {
            break;
        }
        entregar(id, corregidos.remove(&id), destino, &mut hechos);
    }
    if parar.load(Ordering::Relaxed) {
        let _ = destino.entregar(None, en(0, false, true));
        return Err("el usuario lo paro".into());
    }
    if let Some(e) = error_entrega {
        let _ = destino.entregar(None, en(0, false, true));
        return Err(e);
    }
    let mut hechos: Vec<Hecho> = hechos.into_iter().flatten().collect();
    for r in rehechos_diagrama {
        if let Some(h) = hechos.iter_mut().find(|h| h.zona == r.zona) {
            *h = r;
        }
    }

    // 3b. **Un recorte entero, no a parches** (15-oct): un dibujo que no se
    //     pudo dibujar (una grafica escaneada) sale como el recorte de la
    //     imagen; las rayas y los rotulos que el seccionado partio de el
    //     (los ejes, la «x», «f(x)») van dentro de ESE recorte, no aparte.
    let recortes: Vec<usize> = hechos
        .iter()
        .filter(|h| matches!(s.zonas[h.zona].tipo, Tipo::Dibujo | Tipo::Grafica) && h.nota.contains("recorte de la imagen"))
        .map(|h| h.zona)
        .collect();
    let mut tragadas: std::collections::HashSet<usize> = Default::default();
    for &id in &recortes {
        if tragadas.contains(&id) {
            continue;
        }
        let mut caja = s.zonas[id].caja;
        let mut dentro: Vec<usize> = Vec::new();
        // Lo que cae dentro o a caballo (y no es otra figura grande): dentro.
        loop {
            let mas: Vec<usize> = s
                .zonas
                .iter()
                .filter(|z| z.id != id && !dentro.contains(&z.id) && !tragadas.contains(&z.id))
                .filter(|z| matches!(z.tipo, Tipo::Linea | Tipo::Parrafo | Tipo::Formula | Tipo::Dudosa | Tipo::Dibujo | Tipo::Grafica))
                .filter(|z| z.caja.dentro_de(&caja.crecer(s.alto_letra)) >= 0.6 || (z.tipo == Tipo::Linea && z.caja.comun(&caja) > 0.0))
                .filter(|z| z.caja.area() <= 2.0 * s.zonas[id].caja.area())
                .map(|z| z.id)
                .collect();
            if mas.is_empty() {
                break;
            }
            for k in mas {
                let c = s.zonas[k].caja;
                caja = Caja::de_esquinas(caja.x.min(c.x), caja.y.min(c.y), caja.x1().max(c.x1()), caja.y1().max(c.y1()));
                dentro.push(k);
            }
        }
        if dentro.is_empty() {
            continue;
        }
        let e = entrada(id);
        let Some((v, img)) = montaje::foto_recortada(&e, &caja.crecer(2.0)) else { continue };
        let h = Hecho { zona: id, tipo: s.zonas[id].tipo, via: Via::Imagen, elementos: vec![v], imagenes: vec![img], nota: format!("recorte de la imagen, entero (con {} zona(s) de dentro)", dentro.len()) };
        if destino.entregar(Some(&h), en(id, true, false)).is_err() {
            continue;
        }
        tragadas.insert(id);
        if let Some(x) = hechos.iter_mut().find(|x| x.zona == id) {
            *x = h;
        }
        for k in dentro {
            tragadas.insert(k);
            let vacio = Hecho { zona: k, tipo: s.zonas[k].tipo, via: Via::Imagen, elementos: Vec::new(), imagenes: Vec::new(), nota: format!("dentro del recorte de la zona {id}") };
            if destino.entregar(Some(&vacio), en(k, true, false)).is_ok()
                && let Some(x) = hechos.iter_mut().find(|x| x.zona == k)
            {
                *x = vacio;
            }
        }
    }

    // 4. Comprobar y rehacer lo que se parece poco (una vez).
    let tc = Instant::now();
    let mut parecidos = Vec::new();
    let mut rehechas = 0;
    if o.comprobar {
        match pintar(&hechos, tam, p.ancho) {
            Ok(pintado) => {
                for h in hechos.iter_mut() {
                    // (Una zona vaciada porque va en un bloque de texto rico: nada que comparar.)
                    if h.elementos.is_empty() {
                        continue;
                    }
                    let z = &s.zonas[h.zona];
                    // Con aire: una letra un poco mas alta o mas baja no es otra cosa.
                    let caja = z.caja.crecer(s.alto_letra * 0.6);
                    let original = imagen::recortar(&p.rgba, p.ancho, p.alto, &caja);
                    let nuestro = imagen::recortar(&pintado.0, pintado.1, pintado.2, &caja);
                    let v = comparar::parecido((&original.0, original.1, original.2), (&nuestro.0, nuestro.1, nuestro.2));
                    parecidos.push((h.zona, v));
                    // Una formula que la nube leyo y se compuso es mas
                    // ancha o mas alta que la del papel (otra letra, la llave
                    // dibujada) y el parecido baja sin que este mal: solo se
                    // cambia por la imagen si casi no se parece (banco del
                    // 15-oct: el sistema b) volvia a salir como recorte).
                    let leida = h.tipo == Tipo::Formula && matches!(leidos.get(&h.zona), Some(Leido::Latex(_)));
                    // Un diagrama (la revision con ojos de cada figura, 15-oct): si su
                    // dibujo exacto casi no se parece al papel, se pone el recorte
                    // de la imagen (mejor el original que una figura equivocada).
                    let umbral = if leida { o.rehacer_bajo * 0.5 } else if h.tipo == Tipo::Diagrama { o.rehacer_bajo * 0.6 } else { o.rehacer_bajo };
                    // (Un bloque de texto rico se queda: sus zonas ya van dentro.)
                    // (Ni el texto de una lamina: cambiarlo por su recorte era «pegar una captura».)
                    let texto_de_lamina = o.rico && matches!(h.tipo, Tipo::Parrafo | Tipo::Encabezado);
                    if v >= umbral || texto_de_lamina || matches!(h.tipo, Tipo::Linea | Tipo::Foto | Tipo::Recuadro) || h.via == Via::Imagen || h.nota.starts_with("texto rico") {
                        continue;
                    }
                    let otra = match h.tipo {
                        Tipo::Parrafo | Tipo::Encabezado | Tipo::Tabla if h.via != Via::Fiel => Via::Fiel,
                        _ => Via::Imagen,
                    };
                    let nuevo = montaje::hacer(&entrada(z.id), leidos.get(&h.zona), otra);
                    tracing::info!(zona = h.zona, parecido = v, antes = %h.nota, ahora = %nuevo.nota, "hoja al lienzo: zona rehecha");
                    if destino.entregar(Some(&nuevo), en(h.zona, true, false)).is_ok() {
                        *h = nuevo;
                        rehechas += 1;
                    }
                }
            }
            Err(e) => tracing::warn!(%e, "hoja al lienzo: no se pudo pintar para comprobar"),
        }
    }
    // 5. Un recorte que tapa un texto que dice lo mismo (el renglon leido
    //    dos veces: banco del 15-oct, «Indica para que valores...» salia
    //    como imagen y como texto encima): se quita el recorte.
    let es_texto = |h: &Hecho| matches!(h.tipo, Tipo::Parrafo | Tipo::Encabezado) && h.via != Via::Imagen;
    let sobra: Vec<usize> = hechos
        .iter()
        .filter(|h| h.via == Via::Imagen || h.nota.contains("imagen"))
        .filter(|h| !matches!(h.tipo, Tipo::Foto))
        .filter(|h| {
            let c = &s.zonas[h.zona].caja;
            let tapado: f64 = hechos.iter().filter(|o| o.zona != h.zona && es_texto(o)).map(|o| s.zonas[o.zona].caja.comun(c)).sum();
            tapado >= 0.3 * c.area().max(1.0)
        })
        .map(|h| h.zona)
        .collect();
    for id in &sobra {
        let vacio = Hecho { zona: *id, tipo: Tipo::Dudosa, via: Via::Imagen, elementos: Vec::new(), imagenes: Vec::new(), nota: "tapado por un texto: quitado".into() };
        if destino.entregar(Some(&vacio), en(*id, true, false)).is_ok() {
            tracing::info!(zona = id, "hoja al lienzo: recorte tapado por texto, quitado");
        }
    }
    hechos.retain(|h| !sobra.contains(&h.zona));
    // 6. **Nada dos veces** (17-oct, «varias zonas son segmentadas varias
    //    veces y se superponen»): dos zonas que no son recuadro y que se pisan
    //    en mas de la mitad de la menor son la misma cosa hecha dos veces; se
    //    queda la que salio antes en el orden de lectura (la mayor si es
    //    imagen contra texto: el texto ya va tapado en ella, `montaje::foto`).
    let caja_de = |h: &Hecho| s.zonas[h.zona].caja;
    let mut dobles: Vec<usize> = Vec::new();
    for (i, a) in hechos.iter().enumerate() {
        if a.elementos.is_empty() || a.tipo == Tipo::Recuadro || dobles.contains(&a.zona) {
            continue;
        }
        for b in hechos[i + 1..].iter() {
            if b.elementos.is_empty() || b.tipo == Tipo::Recuadro || dobles.contains(&b.zona) {
                continue;
            }
            let (ca, cb) = (caja_de(a), caja_de(b));
            let menor = ca.area().min(cb.area()).max(1.0);
            let texto = |h: &Hecho| matches!(h.tipo, Tipo::Parrafo | Tipo::Encabezado);
            // (Un texto dentro de una figura es su rotulo; dentro de una imagen ya esta tapado.)
            // (De tamano parecido: una formula dentro de un dibujo grande es suya,
            // no la misma cosa; quitarla dejaba vacias las cajitas de «MN = (b+a)/2».)
            let parecidas = ca.area().max(cb.area()) <= 2.5 * menor;
            if parecidas && ca.comun(&cb) > 0.5 * menor && texto(a) == texto(b) {
                dobles.push(b.zona);
                continue;
            }
            // Una formula suelta dentro de un renglon de texto («13 y» de «Si
            // AD=13 y»): el renglon ya la dice (corregido por la nube).
            let (f, t) = if b.tipo == Tipo::Formula && texto(a) { (b, a) } else if a.tipo == Tipo::Formula && texto(b) { (a, b) } else { continue };
            let cf = caja_de(f);
            let en_renglon = s.zonas[t.zona].renglones.iter().any(|r| r.caja.comun(&cf) > 0.5 * cf.area().max(1.0));
            if en_renglon && !dobles.contains(&f.zona) {
                dobles.push(f.zona);
            }
        }
    }
    for id in &dobles {
        let vacio = Hecho { zona: *id, tipo: Tipo::Dudosa, via: Via::Imagen, elementos: Vec::new(), imagenes: Vec::new(), nota: "la misma cosa que otra zona: quitada".into() };
        if destino.entregar(Some(&vacio), en(*id, true, false)).is_ok() {
            tracing::info!(zona = id, "hoja al lienzo: zona repetida, quitada");
        }
    }
    hechos.retain(|h| !dobles.contains(&h.zona));
    let ms_comprobar = tc.elapsed().as_millis();
    tracing::info!(
        zonas = s.zonas.len(),
        peticiones = preguntas.len(),
        trabajadores,
        a_la_vez = max_en_vuelo.load(Ordering::SeqCst),
        ms_primera = ?ms_primera,
        ms_total = t0.elapsed().as_millis() as u64,
        "hoja_a_lienzo: hecha"
    );
    destino.entregar(None, en(0, false, true))?;
    destino.esperar(parar)?;
    Ok(Informe {
        peticiones: preguntas.len(),
        trabajadores,
        a_la_vez: max_en_vuelo.load(Ordering::SeqCst),
        seccionado: s,
        hechos,
        parecidos,
        rehechas,
        ms_seccionar,
        ms_primera,
        ms_total: t0.elapsed().as_millis(),
        ms_comprobar,
        fallos,
    })
}

/// **Lo hecho, pintado** como lo veria el lienzo (escena → PDF de vectores
/// → pixeles de Windows), a `ancho` pixeles: con la escala de la hoja, sus
/// pixeles caen sobre los del original.
pub fn pintar(hechos: &[Hecho], tam: (f32, f32), ancho: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let elementos: Vec<serde_json::Value> = hechos.iter().flat_map(|h| h.elementos.iter().cloned()).collect();
    let imagenes: HashMap<u64, &montaje::ImagenNueva> = hechos.iter().flat_map(|h| h.imagenes.iter()).map(|i| (i.id, i)).collect();
    let mut escena = pixpin_motor2d::Escena::nueva();
    if !elementos.is_empty() {
        let l = pixpin_motor2d::excalidraw::leer(&pixpin_ia::lienzo::documento(elementos)).map_err(|e| e.to_string())?;
        for e in l.elementos() {
            escena.anadir(e);
        }
    }
    let mut hojas = pixpin_motor2d::exportar::hojas(&escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    if hojas.is_empty() {
        return Ok((vec![255; (ancho * ((ancho as f32 * tam.1 / tam.0) as u32).max(1) * 4) as usize], ancho, ((ancho as f32 * tam.1 / tam.0) as u32).max(1)));
    }
    hojas.truncate(1);
    hojas[0].caja = (0.0, 0.0, tam.0, tam.1);
    let resolver = |id: u64| imagenes.get(&id).map(|i| pixpin_pdf::escribir::Pixeles { ancho: i.ancho, alto: i.alto, rgba: i.rgba.clone() });
    let pdf = pixpin_pdf::escribir::de_hojas_a_medida(&hojas, &[tam], &resolver, letra_segoe()).ok_or("no salio el PDF")?;
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ruta = std::env::temp_dir().join(format!("pixpin-hoja-{}-{}.pdf", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::write(&ruta, pdf).map_err(|e| e.to_string())?;
    let r = pixpin_pdf::Documento::abrir(&ruta).and_then(|d| d.renderizar(0, ancho)).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&ruta);
    let img = r?;
    Ok((img.pixeles, img.ancho, img.alto))
}

/// **El motor con vision para las zonas**: el de la nube elegido en
/// Ajustes, en modo ligero. `None` con el local (la herramienta no se hace).
pub fn motor_de_vision() -> Option<Arc<dyn Motor>> {
    let u = crate::oscuro::ubicacion()?;
    motor_de_vision_en(u.raiz())
}

/// Lo mismo con los ajustes de los datos de `raiz` (la cola de encargos
/// sabe cuales son los suyos).
pub fn motor_de_vision_en(raiz: &Path) -> Option<Arc<dyn Motor>> {
    match super::nube::eleccion_reciente(raiz) {
        super::nube::Eleccion::Nube(p) => {
            let clave = pixpin_ia::nube::claves::leer(p)?;
            Some(Arc::new(pixpin_ia::nube::MotorNube::nuevo(p, clave).ligero()))
        }
        super::nube::Eleccion::Local => None,
    }
}

/// **¿Calcar o pasar como lamina?** (15-oct). Lo decide quien EJECUTA
/// `calcar_imagen` —el ejecutor del chat y el de la cola de encargos, el
/// mismo criterio en los dos— y no el prompt: en la app real la lamina de
/// geometria salia «calcada con 115 elementos» porque el desvio solo estaba
/// en un camino. Con un modelo con vision y una imagen entera (sin `zona`)
/// que es lamina (texto y figuras), la pagina leida; si no, `None` y se calca.
pub fn lamina_de_calcar(ruta: &Path, p: &pixpin_ia::calcar::Parametros, vision: Option<&Arc<dyn Motor>>) -> Option<Pagina> {
    vision?;
    if p.zona.is_some() {
        return None;
    }
    let pagina = leer(ruta, None).ok()?;
    let si = es_lamina(&pagina);
    tracing::info!(lamina = si, ruta = %ruta.display(), "calcar_imagen: lamina o dibujo");
    si.then_some(pagina)
}

/// El adjunto de la hoja: por su nombre, o la ultima foto o PDF del chat.
pub fn elegir<'a>(adjuntos: &'a [super::turno::Adjunto], nombre: Option<&str>) -> Result<&'a super::turno::Adjunto, String> {
    let validos: Vec<super::turno::Adjunto> = adjuntos
        .iter()
        .filter(|a| super::ocr::es_imagen(&a.ruta) || super::ocr::es_pdf(&a.ruta))
        .cloned()
        .collect();
    if validos.is_empty() {
        return Err("no hay ninguna foto ni PDF adjunto con la hoja; pide al usuario que la adjunte".into());
    }
    let elegido = super::ocr::elegir(&validos, nombre).or_else(|_| super::ocr::elegir(&validos, None))?;
    adjuntos.iter().find(|a| a.ruta == elegido.ruta).ok_or_else(|| "la hoja ya no esta".into())
}

/// **La herramienta entera** desde el ejecutor del chat: elegir el adjunto,
/// leerlo, pasarlo y contar lo que salio.
#[allow(clippy::too_many_arguments)]
pub fn hacer(
    args: &serde_json::Value,
    adjuntos: &[super::turno::Adjunto],
    vision: Option<Arc<dyn Motor>>,
    semilla: u64,
    parar: &AtomicBool,
    destino: &mut dyn Destino,
    progreso: &mut dyn FnMut(usize, usize),
) -> Result<String, String> {
    let Some(vision) = vision else {
        return Err(hoja::SOLO_NUBE.into());
    };
    let pedido = hoja::Pedido::de(args);
    let a = elegir(adjuntos, pedido.archivo.as_deref())?;
    let p = leer(&a.ruta, pedido.pagina)?;
    let mut o = Opciones::default();
    if let Some(w) = pedido.ancho {
        o.ancho = w;
    }
    contar(&p, &pasar(&p, Some(vision), &o, pedido.fiel, semilla, parar, destino, progreso)?)
}

/// Lo que se le cuenta al modelo de una hoja pasada.
pub fn contar(p: &Pagina, inf: &Informe) -> Result<String, String> {
    tracing::info!(
        zonas = inf.hechos.len(),
        ms_leer = p.ms_leer,
        ms_seccionar = inf.ms_seccionar,
        ms_primera = ?inf.ms_primera,
        ms_total = inf.ms_total,
        rehechas = inf.rehechas,
        "hoja al lienzo"
    );
    let mut r = hoja::resumen(&inf.seccionado, inf.hechos.len(), inf.rehechas, inf.ms_total + p.ms_leer);
    let diagramas = inf.hechos.iter().filter(|h| h.tipo == Tipo::Diagrama).count();
    if inf.trabajadores > 0 {
        r.push_str(&format!("; {} consulta(s) a la nube con {} trabajadores ({} a la vez)", inf.peticiones, inf.trabajadores, inf.a_la_vez));
    }
    if diagramas > 0 {
        r.push_str(&format!("; {diagramas} figura(s) con rectas, circunferencias, arcos y puntos exactos"));
    }
    if !inf.fallos.is_empty() {
        r.push_str(&format!(" ({} zona(s) que el modelo no leyo van como imagen)", inf.fallos.len()));
    }
    Ok(r)
}

/// **Si una imagen es una lamina** (texto y figuras: un apunte, una
/// diapositiva, un problema de geometria) y no un dibujo o una foto: lo que
/// se pide «calcar» se pasa entonces con figuras exactas y texto (como una
/// hoja), no con manchas de color. Con texto (4 palabras o mas) o con una
/// figura que es diagrama; una foto (sin papel liso) nunca.
pub fn es_lamina(p: &Pagina) -> bool {
    let s = hoja::seccion::seccionar(&p.rgba, p.ancho, p.alto, p.palabras.clone());
    let area = (p.ancho as f64 * p.alto as f64).max(1.0);
    let fotos: f64 = s.zonas.iter().filter(|z| z.tipo == Tipo::Foto).map(|z| z.caja.area()).sum();
    if fotos >= 0.6 * area {
        return false;
    }
    let palabras = p.palabras.iter().filter(|w| w.texto.chars().filter(|c| c.is_alphanumeric()).count() >= 2).count();
    if palabras >= 4 {
        return true;
    }
    let o = pixpin_ia::diagrama::Opciones { alto_letra: s.alto_letra, ..Default::default() };
    pixpin_ia::diagrama::vectorizar(&p.rgba, p.ancho, p.alto, &p.palabras, &o, None).es_diagrama()
}

/// **Sin la ventana del chat** (el pedido `asistente` de Flow Launcher, una
/// automatizacion): cada zona directa al lienzo del asistente en el chat
/// `proyecto`, sin cola (nadie la vaciaria).
pub struct SinVentana {
    pub ubicacion: pixpin_store::Ubicacion,
    pub proyecto: String,
    /// El lienzo donde quedo (su referencia), para «Abrir lienzo».
    pub lienzo: Option<String>,
}

impl Destino for SinVentana {
    fn entregar(&mut self, h: Option<&Hecho>, en: EnHoja) -> Result<(), String> {
        let (documento, imagenes) = documento_de(h);
        let (_, referencia) = crate::ventana_chat::sin_ventana::al_lienzo_hoja(&self.ubicacion, &self.proyecto, &documento, &en, &imagenes)?;
        self.lienzo = Some(referencia);
        Ok(())
    }
}

/// La letra de la pantalla para el PDF con que se comprueba (con sus
/// letras de reserva, las ecuaciones no salen con huecos). Se lee una vez.
pub fn letra_segoe() -> Option<&'static pixpin_pdf::letra::Letra> {
    static LETRA: std::sync::OnceLock<Option<pixpin_pdf::letra::Letra>> = std::sync::OnceLock::new();
    LETRA.get_or_init(|| pixpin_pdf::letra::del_sistema("Segoe UI")).as_ref()
}

/// **El lector de las letras sueltas de un diagrama** (`pixpin_ia::diagrama`):
/// el OCR de Windows sobre la hoja de letras que arma el diagrama (una sola
/// llamada para todas). Sin OCR, no lee nada (las letras van como trazos).
pub fn lector_de_letras() -> impl Fn(&[u8], u32, u32) -> Vec<PalabraHoja> + Sync {
    |rgba: &[u8], w: u32, h: u32| ocr(rgba, w, h).unwrap_or_default()
}

/// **Los bloques de texto de una lamina** (15-oct): las zonas de texto y de
/// formula que van una debajo de otra en la misma columna (la «RESOLUCION»:
/// renglones de texto con formulas entre medias) son un bloque, que la nube
/// transcribe en texto rico y sale como UN texto del lienzo. Devuelve, por
/// bloque, su primera zona (la que lo lleva), sus zonas y la zona juntada
/// (caja y renglones de todas). Una zona de texto sola tambien es bloque (su
/// cursiva, su subrayado); una formula sola solo si tiene signos de formula
/// (una cabecera con un logo leida como «formula» se queda como estaba).
pub(crate) fn bloques_de_texto(s: &Seccionado) -> Vec<(usize, Vec<usize>, hoja::Zona)> {
    let a = s.alto_letra.max(6.0);
    let de_dibujo = |z: &hoja::Zona| s.zonas.iter().any(|o| o.id != z.id && matches!(o.tipo, Tipo::Dibujo | Tipo::Grafica | Tipo::Diagrama) && z.caja.dentro_de(&o.caja) >= 0.5);
    // (Una letra suelta no es un bloque: es el rotulo de una figura, la «C».)
    let letras = |z: &hoja::Zona| z.renglones.iter().map(|r| r.texto.chars().filter(|c| c.is_alphanumeric()).count()).sum::<usize>();
    // (Ni unos rotulos sueltos que el OCR junto en un renglon con huecos grandes:
    // «Ene   Feb   Mar» bajo las barras de una grafica van cada uno en su sitio.)
    let esparcida = |z: &hoja::Zona| z.renglones.iter().any(|r| r.palabras.windows(2).any(|w| w[1].caja.x - w[0].caja.x1() > 2.5 * a.max(r.caja.h)));
    let mut ids: Vec<usize> = s.zonas.iter().filter(|z| matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Formula) && !de_dibujo(z) && letras(z) >= 3 && !esparcida(z)).map(|z| z.id).collect();
    ids.sort_by(|&x, &y| s.zonas[x].caja.y.total_cmp(&s.zonas[y].caja.y));
    let mut padre: Vec<usize> = (0..s.zonas.len()).collect();
    fn raiz(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for (k, &i) in ids.iter().enumerate() {
        for &j in &ids[k + 1..] {
            let (ci, cj) = (&s.zonas[i].caja, &s.zonas[j].caja);
            let hueco = cj.y - ci.y1();
            let solape = ci.x1().min(cj.x1()) - ci.x.max(cj.x);
            // (Un titulo no se junta con el parrafo de debajo.)
            let alto = |k: usize| { let mut v: Vec<f64> = s.zonas[k].renglones.iter().map(|r| r.caja.h).collect(); v.sort_by(f64::total_cmp); v.get(v.len() / 2).copied().unwrap_or(a) };
            let (hi, hj) = (alto(i), alto(j));
            if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
                eprintln!("bloque? {i}-{j} hueco {hueco:.0} solape {solape:.0} altos {hi:.0} {hj:.0}");
            }
            let titulo = s.zonas[i].tipo == Tipo::Encabezado || s.zonas[j].tipo == Tipo::Encabezado || hi.max(hj) > 1.35 * hi.min(hj).max(1.0);
            // (Ni lo de dos recuadros distintos: cada tarjeta es suya.)
            let mismo_recuadro = panel_de(s, ci) == panel_de(s, cj);
            if !titulo && mismo_recuadro && hueco >= -0.5 * a && hueco <= 2.0 * a && solape >= 0.3 * ci.w.min(cj.w) {
                let (ri, rj) = (raiz(&mut padre, i), raiz(&mut padre, j));
                if ri != rj {
                    padre[rj] = ri;
                }
            }
        }
    }
    let mut grupos: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for &i in &ids {
        let r = raiz(&mut padre, i);
        grupos.entry(r).or_default().push(i);
    }
    let de_formula = |z: &hoja::Zona| z.renglones.iter().any(|r| r.texto.contains(['=', '∢', '<', '°', '^', '+', '√']));
    let mut salida = Vec::new();
    for (_, mut g) in grupos {
        g.sort_by(|&x, &y| s.zonas[x].caja.y.total_cmp(&s.zonas[y].caja.y));
        if g.len() == 1 && s.zonas[g[0]].tipo == Tipo::Formula && !de_formula(&s.zonas[g[0]]) {
            continue;
        }
        let mut z = s.zonas[g[0]].clone();
        z.tipo = Tipo::Parrafo;
        z.renglones = g.iter().flat_map(|&k| s.zonas[k].renglones.iter().cloned()).collect();
        for &k in &g[1..] {
            let c = &s.zonas[k].caja;
            z.caja = Caja::de_esquinas(z.caja.x.min(c.x), z.caja.y.min(c.y), z.caja.x1().max(c.x1()), z.caja.y1().max(c.y1()));
        }
        salida.push((g[0], g, z));
    }
    salida
}

/// El recuadro mas pequeno donde cae una caja (`None`: en la hoja).
fn panel_de(s: &Seccionado, c: &Caja) -> Option<usize> {
    (0..s.paneles.len()).filter(|&i| s.paneles[i].contiene(c.cx(), c.cy())).min_by(|&x, &y| s.paneles[x].area().total_cmp(&s.paneles[y].area()))
}

/// **Lo que el OCR vio de color** en un bloque, para la nube: «"m<AOC=a"
/// rojo; "(a + b)" rojo; ...». Haiku se olvidaba de los colores; con la
/// pista los pone donde van (las letras las lee el, mejor que el OCR).
pub(crate) fn pista_de_colores(z: &hoja::Zona) -> String {
    let nombre = |c: [u8; 3]| -> Option<&'static str> {
        let (r, g, b) = (c[0] as f32, c[1] as f32, c[2] as f32);
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        if mx - mn < 45.0 {
            return None;
        }
        let h = if mx == r {
            ((g - b) / (mx - mn)).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / (mx - mn) + 2.0
        } else {
            (r - g) / (mx - mn) + 4.0
        } * 60.0;
        Some(match h {
            h if !(20.0..340.0).contains(&h) => "rojo",
            h if h < 45.0 => "naranja",
            h if h < 170.0 => "verde",
            h if h < 260.0 => "azul",
            _ => "morado",
        })
    };
    let mut v: Vec<String> = Vec::new();
    for r in &z.renglones {
        for p in &r.palabras {
            if let Some(n) = p.color.and_then(nombre)
                && p.texto.chars().any(|c| c.is_alphanumeric())
            {
                v.push(format!("\"{}\" {n}", p.texto));
            }
        }
    }
    v.truncate(40);
    v.join("; ")
}
