//! **Un Word, un libro o un PDF anotado, hecho pagina web** en la que se
//! sigue anotando. Puerto de `motor/DocumentoAnotado.kt`,
//! `ui/ExportarDocumentoAnotado.kt` y `ui/ExportarPdfAnotado.kt` del movil
//! (20/21-sep-2026).
//!
//! Lo pidio el usuario: lo exportado tiene que llevar **lo anotado y los
//! marcadores**, y tener las mismas funciones que la web del movil (anotar,
//! guardarse, imprimir...). Antes la hoja de compartir convertia el Word en
//! dibujos de sus paginas: sin texto de verdad, y con la tinta clavada al
//! pixel. Ahora:
//!
//! - **Word, libro, pagina, Markdown**: el documento como HTML real
//!   (`pixpin_docs::documento::cuerpo_para_anotar`), con la letra, la columna
//!   y el papel del lector; la tinta encima, en SVG vectorial, **por piezas
//!   atadas a su bloque**; los marcadores con su emoticono en el canto y en un
//!   riel que lleva a ellos.
//! - **PDF**: las hojas una debajo de otra, con su margen a cada lado: como
//!   **lineas** (SVG, `pixpin_pdf::plano_web`) si la hoja es vectorial, y
//!   como fotografia solo si es un escaneo o trae algo que no se entiende; lo anotado encima de cada una (tambien lo de los
//!   margenes) y los marcadores saltando a su hoja y a su altura.
//!
//! Las dos son la misma clase de hoja de la pagina web de siempre
//! (`exportar_html::HojaDocumento`), asi que los mandos le vienen dados por
//! el guion del movil: lapiz, resaltador, goma, colores, deshacer, guardar
//! con lo nuevo, imprimir con lo anotado, presentar y zoom.
//!
//! **Desde el chat y desde el lector es lo mismo** (`ExportarDocumentoAnotado`
//! del movil): todo lo que hace falta esta en disco junto al documento —la
//! capa (`.pixpin-anotado/`), la letra y los marcadores (`.pixpin-lectura`)—
//! y la disposicion se vuelve a medir con el mismo DirectWrite del lector.
//! Si no se puede medir (sin tarjeta grafica), valen **las medidas que dejo
//! el lector al salir** ([`guardar_medidas`], `guardarMedidas` del movil); y
//! si nunca se midio, lo anotado sale donde estaba y los marcadores por su
//! fraccion del alto.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_docs::documento::{Documento, Imagen};
use pixpin_docs::{lectura, vista};
use pixpin_motor2d::exportar::{self as ex, Alcance};
use pixpin_motor2d::exportar_html::documento::{self as dw, HOLGURA, Pieza, Senal};
use pixpin_motor2d::exportar_html::{self, HojaDeLaPagina, HojaDocumento};
use pixpin_motor2d::exportar_svg::{self, OpcionesSvg};
use pixpin_motor2d::{ColorRgba, Escena};

use crate::visor::Colocado;

/// Lo que mide una hoja de PDF en la pagina web, en pixeles CSS (la del
/// movil). La tinta la mide en `vista::ANCHO_HOJA` unidades.
const COLUMNA_PDF: f32 = 800.0;
/// La calidad del JPEG de cada hoja del PDF. Una hoja de texto a 80 pesaba
/// mas de medio mega; a 72 la letra sigue limpia al doble de aumento y un
/// articulo de treinta hojas baja de los diez megas.
const CALIDAD_PDF: u8 = 72;
/// El aire entre dos hojas del PDF.
const ENTRE_HOJAS: f32 = 8.0;
/// El guion que despliega en SVG las hojas que viajan como lineas.
const PLANO_EN_SVG: &str = include_str!("plano_en_svg.js");
/// El papel alrededor de las hojas del PDF: el de sus margenes en el lector.
const PAPEL_PDF: &str = "#f3f3f0";
/// Lo mas ancha que va una foto del documento: mas de lo que ninguna
/// columna ensena solo es peso.
const ANCHO_DE_FOTO: u32 = 1100;
/// Por debajo de esto una foto va tal cual: recomprimirla no ahorra nada.
const FOTO_LIGERA: usize = 150_000;
/// El ancho de papel que cabe en un A4 con los margenes de impresion de la
/// pagina (10 mm): 190 mm a 96 ppp.
const ANCHO_IMPRESO: f32 = 718.0;

// ---------------------------------------------------------------------------
// Las medidas

/// **A que altura cae cada bloque** del documento (los de
/// `cuerpo_para_anotar`, uno a uno) y cuanto mide de alto, en unidades del
/// lector: lo que el movil sacaba con `DocumentoAnotado.MEDIR`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Medidas {
    pub tops: Vec<f32>,
    pub alto: f32,
}

/// **Las alturas de los bloques, sacadas de la disposicion del lector**
/// (`visor::colocar`). La cabecera va primero (lo que el lector coloca sin
/// bloque), y luego un valor por bloque. Un bloque vacio el lector no lo
/// coloca, pero en la pagina ocupa su hueco: se le da la altura donde
/// empezaria, detras del anterior y con su aire.
pub(crate) fn medidas_de(doc: &Documento, colocados: &[Colocado], alto: f32, base: f32) -> Medidas {
    let cabecera = pixpin_docs::documento::bloques_para_anotar(doc) - doc.bloques.len();
    let mut tops = Vec::with_capacity(cabecera + doc.bloques.len());
    let mut abajo = base * 2.0;
    let mut i = 0usize;
    while i < colocados.len() && colocados[i].bloque.is_none() && tops.len() < cabecera {
        tops.push(colocados[i].y);
        abajo = colocados[i].y + colocados[i].alto;
        i += 1;
    }
    // Un Word no lleva cabecera en la maqueta del movil (K16): el titulo y
    // el autor de la pagina web van antes que todo, un pixel cada uno.
    if tops.is_empty() {
        abajo = 0.0;
    }
    while tops.len() < cabecera {
        tops.push(abajo);
        abajo += 1.0;
    }
    for (n, b) in doc.bloques.iter().enumerate() {
        // Los colocados van en orden: el de este bloque, si lo hay, es el
        // siguiente que tenga bloque.
        while i < colocados.len() && colocados[i].bloque.is_some_and(|k| k < n) {
            i += 1;
        }
        match colocados.get(i).filter(|c| c.bloque == Some(n)) {
            Some(c) => {
                tops.push(c.y);
                let (_, aire, _) = crate::visor::pinta(b.clase);
                // Una raya ocupa su aire detras, no su pixel.
                abajo = if c.texto.is_empty() && c.caja.is_none() { c.y + base * aire } else { c.y + c.alto };
            }
            None => {
                let (_, aire, _) = crate::visor::pinta(b.clase);
                let y = abajo + base * aire;
                tops.push(y);
                abajo = y + base * 0.6;
            }
        }
    }
    Medidas { tops, alto }
}

/// Con que letra se midio: medidas de otra letra no sirven.
fn letra_de(ajustes: &lectura::Ajustes) -> String {
    format!("{}:{}:{}", ajustes.tamano, ajustes.tipo, ajustes.grosor)
}

/// Donde deja el lector las medidas: con la tinta, junto al documento.
pub(crate) fn ruta_de_medidas(documento: &Path) -> PathBuf {
    crate::lector_tinta::carpeta_de(documento).join("medidas.json")
}

/// **Deja las medidas para exportar sin el lector** (`guardarMedidas` del
/// movil). Solo si el documento tiene algo que exportar (tinta, letra
/// fijada o marcadores): leer un Word no tiene por que dejar carpetas al
/// lado.
pub(crate) fn guardar_medidas(
    documento: &Path,
    ajustes: &lectura::Ajustes,
    columna: f32,
    medidas: &Medidas,
    hay_tinta: bool,
) -> std::io::Result<()> {
    if !hay_tinta && !ajustes.letra_fijada() && ajustes.marcadores.is_empty() {
        return Ok(());
    }
    let ruta = ruta_de_medidas(documento);
    if let Some(c) = ruta.parent() {
        std::fs::create_dir_all(c)?;
    }
    let tops: Vec<f64> = medidas.tops.iter().map(|t| (*t as f64 * 10.0).round() / 10.0).collect();
    let json = serde_json::json!({
        "t": tops,
        "h": medidas.alto,
        "c": columna.round(),
        "l": letra_de(ajustes),
    });
    std::fs::write(ruta, json.to_string())
}

/// **Las medidas que dejo el lector** y la columna con la que se tomaron,
/// si son de esta letra y, con la letra fijada, de esta columna. Sin tinta
/// la columna es la que cabia en la pantalla del lector: se exporta con
/// ella, que es como se estaba leyendo cuando se puso cada marcador.
pub(crate) fn medidas_guardadas(documento: &Path, ajustes: &lectura::Ajustes) -> Option<(f32, Medidas)> {
    let texto = std::fs::read_to_string(ruta_de_medidas(documento)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&texto).ok()?;
    if v.get("l")?.as_str()? != letra_de(ajustes) {
        return None;
    }
    let columna = v.get("c")?.as_f64()? as f32;
    if columna <= 0.0 || (ajustes.letra_fijada() && columna.round() as u32 != ajustes.columna) {
        return None;
    }
    let tops = v
        .get("t")?
        .as_array()?
        .iter()
        .filter_map(|x| x.as_f64().map(|f| f as f32))
        .collect();
    let alto = v.get("h")?.as_f64()? as f32;
    Some((columna, Medidas { tops, alto }))
}

/// **Mide el documento ahora**, con el mismo DirectWrite que el lector y
/// fuera de un fotograma: la misma disposicion que la pantalla.
fn medir_ahora(doc: &Documento, ajustes: &lectura::Ajustes, columna: f32, hoja: crate::visor::Hoja) -> Result<Medidas> {
    let dispositivo = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para medir el texto")?;
    let motor = pixpin_render::MotorRender::nuevo(dispositivo.d3d()).context("sin motor para medir el texto")?;
    let mide = |texto: &str, tam: f32, ancho: f32, tramos: &[pixpin_render::Tramo], letra: &crate::visor::Letra| {
        motor.medir_de_lectura(texto, tam, ancho, tramos, &letra.para_pintar())
    };
    let (colocados, alto) = crate::visor::colocar(doc, ajustes, columna, hoja, &mide);
    Ok(medidas_de(doc, &colocados, alto, crate::visor::tamano_base(ajustes)))
}

// ---------------------------------------------------------------------------
// Lo anotado

/// **Lo anotado, por piezas**: los trazos que comparten ancla salen en un
/// mismo SVG (`porAnclas` del movil). `ancla` dice a que bloque va un trazo
/// por la altura de su centro; `(dx, dy, k)` lleva sus unidades a las de la
/// pagina (desplazar y escalar).
pub(crate) fn piezas_de(escena: &Escena, ancla: &dyn Fn(f32) -> i32, (dx, dy, k): (f64, f64, f64)) -> Vec<Pieza> {
    let mut grupos: BTreeMap<i32, Vec<u64>> = BTreeMap::new();
    for e in escena.visibles() {
        let (_, y0, _, y1) = e.caja();
        grupos.entry(ancla((y0 + y1) / 2.0)).or_default().push(e.id);
    }
    let mut piezas = Vec::with_capacity(grupos.len());
    for (a, ids) in grupos {
        let Some(hoja) = ex::hojas(escena, Alcance::Seleccion, &ids, None).into_iter().next() else {
            continue;
        };
        if hoja.ordenes.is_empty() {
            continue;
        }
        let svg = exportar_svg::svg(&hoja, OpcionesSvg { fondo: None, marcos: false }, &|_| None);
        let (x0, y0, _, _) = hoja.caja;
        piezas.push(Pieza {
            svg,
            x: dx + x0 as f64 * k,
            y: dy + y0 as f64 * k,
            ancho: hoja.ancho() as f64 * k,
            alto: hoja.alto() as f64 * k,
            ancla: a,
        });
    }
    piezas
}

// ---------------------------------------------------------------------------
// Word, libros, paginas

fn hex(c: pixpin_render::Color) -> String {
    ex::hex(ColorRgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: 1.0,
    })
}

/// Un numero de pixeles para el CSS, con un decimal.
fn px(v: f32) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

/// **Lo que se imprime**: la columna y, de los margenes, solo hasta donde
/// hay algo anotado. Imprimir los dos margenes enteros dejaria el texto en
/// un tercio del papel.
fn estilo_de_impresion(columna: f32, margen: f32, piezas: &[Pieza], con_marcadores: bool) -> String {
    let mut izq = margen;
    let mut der = margen + columna + if con_marcadores { 32.0 } else { 0.0 };
    for p in piezas {
        izq = izq.min(p.x as f32);
        der = der.max((p.x + p.ancho) as f32);
    }
    let total = columna + 2.0 * margen;
    let izq = (izq - 16.0).max(0.0);
    let der = (der + 16.0).min(total);
    let zoom = (ANCHO_IMPRESO / (der - izq).max(1.0)).min(1.0);
    format!(
        "@media print{{.doc-caja{{transform:none!important;margin:0 0 0 -{}px!important;padding-bottom:0!important;zoom:{:.3};\
         -webkit-print-color-adjust:exact;print-color-adjust:exact}}.pprail{{display:none!important}}}}",
        px(izq),
        zoom
    )
}

/// **La hoja de estilo del documento, como la pinta el lector**: su papel
/// oscuro, su letra (Segoe UI o Consolas, normal o gruesa), el interlineado
/// de DirectWrite y el aire delante de cada bloque (`visor::pinta`). Cuanto
/// mas se parezca, menos tiene que correr el guion lo anotado; lo que no
/// case lo corrige el ancla.
fn estilo_de_texto(ajustes: &lectura::Ajustes, columna: f32, margen: f32, piezas: &[Pieza], con_marcadores: bool) -> String {
    use crate::lector::{APAGADO, FONDO, RAYA, TEXTO};
    use pixpin_docs::documento::Clase;
    let b = crate::visor::tamano_base(ajustes);
    let aire = |c: Clase| px(b * crate::visor::pinta(c).1);
    // Las cuatro letras y los cuatro pesos del movil (K16), con las del
    // navegador de reserva.
    let familia = match ajustes.tipo {
        0 => "'Noto Serif',Georgia,serif",
        2 => "'Courier New',monospace",
        3 => "Caveat,cursive",
        _ => "Roboto,'Segoe UI',system-ui,sans-serif",
    };
    let peso = lectura::PESOS[usize::from(ajustes.grosor).min(lectura::GROSORES - 1)];
    let mut s = String::with_capacity(2048);
    s.push_str(&format!(
        ".doc-caja{{background:{fondo}}}\
         .doc{{display:flow-root;padding:{p}px 0;background:{fondo};color:{texto};font-family:{familia};font-weight:{peso};line-height:{linea}}}\
         .doc :where([data-b]){{margin:{parrafo}px 0 0;padding:0}}\
         .doc h1,.doc h2,.doc h3,.doc h4,.doc h5,.doc h6{{font-weight:700;line-height:{linea}}}\
         .doc h1{{font-size:1.7em;margin-top:{h1}px}}.doc h2{{font-size:1.4em;margin-top:{h2}px}}\
         .doc h3{{font-size:1.2em;margin-top:{h3}px}}.doc h4,.doc h5,.doc h6{{font-size:1.05em;margin-top:{h4}px}}\
         .doc blockquote{{margin-top:{cita}px;padding-left:{sangria_cita}px;color:{apagado}}}\
         .doc pre{{font:inherit;font-size:.9em;white-space:pre-wrap;margin-top:{codigo}px}}\
         .doc .aparte{{font-size:.9em;color:{apagado};margin-top:{nota}px}}\
         .doc .lista{{padding-left:{sangria_lista}px}}\
         .doc .vacio{{height:.6em}}\
         .doc hr{{border:0;border-top:1px solid {raya};height:{hr_alto}px;margin-top:{regla}px;box-sizing:content-box}}\
         .doc .imagen img{{display:block;max-width:100%;height:auto}}\
         .doc b,.doc strong{{font-weight:700}}.doc code{{font-family:Consolas,monospace}}\
         .doc .enlace{{text-decoration:underline;font-style:italic}}",
        fondo = hex(FONDO),
        texto = hex(TEXTO),
        apagado = hex(APAGADO),
        raya = hex(RAYA),
        p = px(b * 2.0),
        linea = ex::INTERLINEA,
        parrafo = aire(Clase::Parrafo),
        h1 = aire(Clase::Titulo(1)),
        h2 = aire(Clase::Titulo(2)),
        h3 = aire(Clase::Titulo(3)),
        h4 = aire(Clase::Titulo(4)),
        cita = aire(Clase::Cita),
        sangria_cita = px(b * 1.4),
        codigo = aire(Clase::Codigo),
        nota = aire(Clase::Nota),
        sangria_lista = px(b * 1.2),
        hr_alto = px((b * crate::visor::pinta(Clase::Regla).1 - 1.0).max(0.0)),
        regla = aire(Clase::Regla),
    ));
    s.push_str(&estilo_de_impresion(columna, margen, piezas, con_marcadores));
    s
}

/// **Un Word o un libro como hoja de la pagina web**, sin tocar el disco:
/// el documento, su letra, lo anotado (`escena`, en unidades del lector con
/// el cero en el borde de la columna) y, si las hay, sus medidas. Sin
/// medidas lo anotado se queda donde estaba y los marcadores van por su
/// fraccion del alto.
pub(crate) fn hoja_de_texto(
    nombre: &str,
    doc: &Documento,
    ajustes: &lectura::Ajustes,
    escena: &Escena,
    columna: f32,
    medidas: Option<&Medidas>,
    imagen: &dyn Fn(&Imagen) -> Option<String>,
) -> HojaDocumento {
    let columna = columna.round().max(1.0);
    let margen = vista::margen_de(columna).round();
    let tops: Vec<f64> = medidas.map(|m| m.tops.iter().map(|t| *t as f64).collect()).unwrap_or_default();
    let alto = medidas.map_or(0.0, |m| m.alto as f64);
    let piezas = piezas_de(escena, &|y| dw::ancla_de(y as f64, &tops, HOLGURA), (margen as f64, 0.0, 1.0));
    let senales: Vec<Senal> = ajustes
        .marcadores
        .iter()
        .map(|m| {
            let y = m.fraccion as f64 * alto;
            Senal {
                emoji: m.emoji.clone(),
                y,
                ancla: if alto > 0.0 { dw::ancla_de(y, &tops, HOLGURA) } else { -1 },
                // Con medidas, la altura manda aunque no haya bloque
                // encima (un marcador en la cabecera): la fraccion es solo
                // para cuando nunca se midio.
                fraccion: if alto > 0.0 { -1.0 } else { m.fraccion as f64 },
            }
        })
        .collect();
    let estilo = estilo_de_texto(ajustes, columna, margen, &piezas, !senales.is_empty());
    HojaDocumento {
        nombre: nombre.to_string(),
        estilo,
        cuerpo: pixpin_docs::documento::cuerpo_para_anotar(doc, imagen),
        capa: dw::capa_de(&piezas, &senales, columna as u32, margen as u32, "d"),
        columna: columna as u32,
        margen: margen as u32,
        tops,
        letra: crate::visor::tamano_base(ajustes) as f64,
        fondo: hex(crate::lector::FONDO),
        bloques: dw::SELECTOR_MARCADO.into(),
        riel: dw::riel_de(&senales, "d"),
    }
}

/// La pagina entera de unas hojas de documento.
pub(crate) fn pagina(hojas: &[HojaDocumento], titulo: &str) -> Option<String> {
    let mixtas: Vec<HojaDeLaPagina<'_>> = hojas.iter().map(HojaDeLaPagina::Documento).collect();
    let nombre = format!("{} (anotado)", exportar_html::nombre_de_fichero(titulo));
    exportar_html::paginas_mixtas(&mixtas, titulo, &nombre, exportar_html::Opciones::default(), None)
}

/// **La pagina web de un Word o un libro con lo anotado**, leyendo todo del
/// disco: lo que usa la hoja de compartir, venga del chat o del lector.
pub(crate) fn web_de_texto(ruta: &Path) -> Result<String> {
    web_de_texto_con(ruta, &medir_ahora)
}

/// [`web_de_texto`] con la forma de medir aparte: las pruebas miden sin
/// tarjeta para comprobar que se cae a lo guardado.
pub(crate) fn web_de_texto_con(
    ruta: &Path,
    medir: &dyn Fn(&Documento, &lectura::Ajustes, f32, crate::visor::Hoja) -> Result<Medidas>,
) -> Result<String> {
    let doc = pixpin_docs::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let capa = crate::anotado_del_adjunto::leer_capa(ruta);
    let guardadas = medidas_guardadas(ruta, &ajustes);
    let columna = if ajustes.letra_fijada() {
        ajustes.columna as f32
    } else {
        guardadas.as_ref().map_or(super::COLUMNA_SIN_ANOTAR, |(c, _)| *c)
    };
    let medidas = match medir(&doc, &ajustes, columna, crate::visor::Hoja::de(ruta)) {
        Ok(m) => Some(m),
        Err(e) => {
            tracing::info!(?e, "no se pudo medir el documento; se usan las medidas que dejo el lector");
            guardadas.map(|(_, m)| m)
        }
    };
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let hoja = hoja_de_texto(&titulo, &doc, &ajustes, &capa.escena, columna, medidas.as_ref(), &imagen_para_la_web);
    pagina(&[hoja], &titulo).context("sin hoja")
}

// ---------------------------------------------------------------------------
// Fotos

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", ex::base64(bytes))
}

/// **Una foto para dentro de la pagina, y a dieta**: JPEG si es opaca (un
/// escaneo, una foto: la quinta parte), PNG si tiene transparencias.
fn foto_para_la_web(img: &ImagenRgba, calidad: u8) -> Option<String> {
    if img.pixeles.chunks_exact(4).all(|p| p[3] == 255) {
        let bytes = pixpin_codec::imagen::codificar_jpg(img, calidad).ok()?;
        return Some(data_uri("image/jpeg", &bytes));
    }
    Some(data_uri("image/png", &pixpin_codec::codificar_png(img).ok()?))
}

/// **Las fotos del documento, dentro de la pagina y a dieta**
/// (`conLasFotosDentro` del movil). Una pagina que se manda sola perderia
/// las sueltas; las grandes —mas anchas de lo que ninguna columna ensena— se
/// encogen y se recomprimen, que es lo que hace que el fichero pese poco
/// aunque el libro traiga fotos de camara. Si no se entiende, va tal cual.
pub(crate) fn imagen_para_la_web(im: &Imagen) -> Option<String> {
    if im.datos.is_empty() {
        return None;
    }
    if im.datos.len() <= FOTO_LIGERA || im.mime.contains("svg") {
        return Some(data_uri(&im.mime, &im.datos));
    }
    let extension = im.mime.rsplit('/').next().unwrap_or("png").replace("jpeg", "jpg");
    let temporal = std::env::temp_dir().join(format!(
        "pixpin-foto-web-{}-{}.{extension}",
        std::process::id(),
        im.datos.len()
    ));
    let encogida = std::fs::write(&temporal, &im.datos)
        .ok()
        .and_then(|_| pixpin_codec::cargar(&temporal).ok())
        .and_then(|img| {
            let img = if img.ancho > ANCHO_DE_FOTO {
                let alto = ((img.alto as u64 * ANCHO_DE_FOTO as u64) / img.ancho as u64).max(1) as u32;
                pixpin_codec::imagen::redimensionar(img, ANCHO_DE_FOTO, alto).ok()?
            } else {
                img
            };
            foto_para_la_web(&img, 78)
        });
    let _ = std::fs::remove_file(&temporal);
    Some(encogida.unwrap_or_else(|| data_uri(&im.mime, &im.datos)))
}

// ---------------------------------------------------------------------------
// PDF

/// Una hoja del PDF para la hoja web: su foto, su alto en la pagina y lo
/// anotado en ella (en unidades del lector).
pub(crate) struct HojaPdf {
    pub foto: Option<String>,
    /// **La hoja como lineas**: el paquete de `pixpin_pdf::plano_web` en
    /// unidades de la columna, si la pagina es vectorial y se entiende
    /// entera. Con el, `foto` no hace falta y no se pinta.
    pub plano: Option<String>,
    /// El alto de la hoja en unidades del lector (a `vista::ANCHO_HOJA` de ancho).
    pub alto: f32,
    pub tinta: Escena,
    /// El numero de la hoja en el PDF, desde 0: el de sus marcas.
    pub pagina: u32,
}

/// **Un PDF anotado como hoja de la pagina web**, sin tocar el disco: las
/// hojas en una columna, una debajo de otra, con su margen de
/// `vista::MARGEN_DEL_PDF` a cada lado; lo anotado encima de cada una,
/// atado a ella; los marcadores en su hoja y a su altura.
pub(crate) fn hoja_de_pdf(nombre: &str, hojas: &[HojaPdf], marcas_texto: &str) -> HojaDocumento {
    let columna = COLUMNA_PDF;
    let margen = (columna * vista::MARGEN_DEL_PDF).round();
    let k = (columna / vista::ANCHO_HOJA) as f64;
    let marcas = pixpin_motor2d::marcas::de_texto(marcas_texto);
    let mut tops = Vec::with_capacity(hojas.len());
    let mut piezas = Vec::new();
    let mut senales = Vec::new();
    let mut cuerpo = String::new();
    let mut y = 0.0f64;
    let mut hay_planos = false;
    for (n, h) in hojas.iter().enumerate() {
        let alto = (h.alto as f64 * k).max(1.0);
        match (&h.plano, &h.foto) {
            // **Como lineas** (`PlanoWeb` del movil): un SVG vacio y su
            // paquete al lado, que el guion despliega al cargar. Se amplia
            // sin grano y se imprime como lineas.
            (Some(json), _) => {
                let id = format!("plano-{n}");
                cuerpo.push_str(&format!(
                    "<svg data-b class=\"plano-hoja\" id=\"{id}\" role=\"img\" aria-label=\"Hoja {}\" \
                     viewBox=\"0 0 {} {}\" preserveAspectRatio=\"xMidYMin meet\" width=\"{}\" height=\"{}\"></svg>\
                     <script type=\"application/json\" class=\"plano-datos\" data-hoja=\"{id}\">{}</script>",
                    h.pagina + 1,
                    columna as u32,
                    px(alto as f32),
                    columna as u32,
                    alto.round() as u32,
                    json.replace("</", "<\\/")
                ));
                hay_planos = true;
            }
            (None, Some(src)) => cuerpo.push_str(&format!(
                "<img data-b alt=\"Hoja {}\" width=\"{}\" height=\"{}\" src=\"{src}\">",
                h.pagina + 1,
                columna as u32,
                alto.round() as u32
            )),
            // Una hoja que no se pudo dibujar ocupa su sitio en blanco: si
            // no, lo anotado de las de abajo subiria a la que no esta.
            (None, None) => cuerpo.push_str(&format!(
                "<img data-b alt=\"Hoja {}\" width=\"{}\" height=\"{}\" style=\"height:{}px\">",
                h.pagina + 1,
                columna as u32,
                alto.round() as u32,
                px(alto as f32)
            )),
        }
        let ancla = n as i32;
        tops.push(y);
        piezas.extend(piezas_de(&h.tinta, &|_| ancla, (margen as f64, y, k)));
        for m in marcas.iter().filter(|m| pixpin_motor2d::marcas::pagina_de(m) == h.pagina) {
            let dentro = pixpin_motor2d::marcas::alto_en_la_pagina(m);
            senales.push(Senal {
                emoji: m.emoji.clone(),
                y: y + dentro * alto,
                ancla,
                fraccion: -1.0,
            });
        }
        y += alto + ENTRE_HOJAS as f64;
    }
    if hay_planos {
        // El guion que despliega las hojas que vinieron como lineas, una
        // vez para todas. Va dentro del cuerpo y no con el armazon: solo lo
        // lleva la pagina que lo necesita.
        cuerpo.push_str("<script>");
        cuerpo.push_str(PLANO_EN_SVG);
        cuerpo.push_str("</script>");
    }
    let mut estilo = format!(
        ".doc-caja{{background:{PAPEL_PDF}}}.doc{{line-height:0;background:{PAPEL_PDF}}}\
         .doc img,.doc svg.plano-hoja{{display:block;width:100%;height:auto;margin:0 0 {}px;background:#fff;break-inside:avoid;page-break-inside:avoid}}",
        ENTRE_HOJAS as u32
    );
    estilo.push_str(&estilo_de_impresion(columna, margen, &piezas, !senales.is_empty()));
    HojaDocumento {
        nombre: nombre.to_string(),
        estilo,
        cuerpo,
        capa: dw::capa_de(&piezas, &senales, columna as u32, margen as u32, "pdf"),
        columna: columna as u32,
        margen: margen as u32,
        tops,
        letra: 16.0,
        fondo: PAPEL_PDF.into(),
        bloques: dw::SELECTOR_MARCADO.into(),
        riel: dw::riel_de(&senales, "pdf"),
    }
}

/// A cuantos pixeles se dibuja cada hoja: cuantas mas hojas, menos, para
/// que un documento largo no pese decenas de megas (el movil: 1600, 1200 y
/// 960). A 1600 una hoja de 800 px CSS se ve nitida en una pantalla de
/// doble densidad.
fn ancho_de_foto(hojas: usize) -> u32 {
    match hojas {
        0..=10 => 1600,
        11..=40 => 1200,
        _ => 960,
    }
}

/// **La pagina web de un PDF con lo anotado**, leyendo todo del disco.
/// `paginas` son las hojas que van (desde 0); `None`, todas.
pub(crate) fn web_de_pdf(ruta: &Path, paginas: Option<&[usize]>) -> Result<String> {
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;
    let colocadas = vista::Hojas::colocar(&doc.medidas());
    let todas: Vec<usize> = (0..colocadas.cuantas()).collect();
    let cuales: Vec<usize> = paginas
        .unwrap_or(&todas)
        .iter()
        .copied()
        .filter(|i| *i < colocadas.cuantas())
        .collect();
    anyhow::ensure!(!cuales.is_empty(), "el PDF no tiene esas hojas");
    let ancho = ancho_de_foto(cuales.len());
    // **Las hojas vectoriales, como lineas** (`PlanoWeb.deArchivo` del
    // movil): nitidas a cualquier aumento y, en un documento de texto, la
    // mitad de peso que su JPEG (medido: 353 KB frente a 731 una hoja de un
    // articulo). Un escaneo, una hoja con algo que no se entiende o un
    // plano pasado de tope vuelven a la foto de siempre. Leer las lineas
    // ademas ahorra pintar: un A0 de AutoCAD tardaba 16 s en salir de
    // Windows y 2,3 s como lineas.
    let planos = std::fs::read(ruta)
        .map(|b| pixpin_pdf::plano_web::de_paginas(&b, &cuales, COLUMNA_PDF as f64))
        .unwrap_or_else(|_| vec![None; cuales.len()]);
    // La tinta de un PDF de proyecto esta en sus hojas (`lector_pdf_proyecto`).
    let donde = crate::lector_pdf_proyecto::DondeVa::solo_leer(ruta);
    let espacios = crate::anotado_del_adjunto::leer(ruta).espacios;
    let hojas: Vec<HojaPdf> = cuales
        .iter()
        .zip(planos)
        .map(|(&i, plano)| {
            let foto = if plano.is_some() {
                None
            } else {
                doc.renderizar(i as u32, ancho)
                    .inspect_err(|e| tracing::warn!(?e, hoja = i, "hoja del PDF que no se pudo dibujar para la web"))
                    .ok()
                    .and_then(|img| foto_para_la_web(&img, CALIDAD_PDF))
            };
            let capa = donde.para_leer(ruta, i);
            let tinta = if capa.is_file() {
                donde.leer_capa(ruta, i, espacios, colocadas.altos[i]).escena
            } else {
                Escena::nueva()
            };
            HojaPdf {
                foto,
                plano,
                alto: colocadas.altos[i],
                tinta,
                pagina: i as u32,
            }
        })
        .collect();
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let hoja = hoja_de_pdf(&titulo, &hojas, &ajustes.marcas);
    pagina(&[hoja], &titulo).context("sin hoja")
}

#[cfg(test)]
mod pruebas;
