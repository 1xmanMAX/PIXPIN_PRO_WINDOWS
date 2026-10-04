//! **Paginas vivas y enlaces a hojas** (H12): una hoja de un proyecto
//! (lienzo, pagina de PDF anotada, tabla, otra nota, foto) metida en una
//! nota como **imagen que se actualiza**, y los enlaces que abren una hoja.
//!
//! # Como es una pagina viva
//!
//! Una foto normal de la nota cuyo fichero se llama `vivo-<hoja>.png`
//! (`pixpin_docs::md_imagen`), con `<hoja>` el codigo unico de la hoja, en
//! `notas/` del proyecto de la nota (o en `adjuntos/` de un `.md` suelto):
//! `![Planta baja](pixpin:files/guardados/pc/<chat>/notas/vivo-K7Q2….png)`.
//! El movil la ve como cualquier foto (la ultima copia viaja con la nota);
//! el PC la vuelve a pintar cuando la hoja cambia. Por que no un titulo
//! `"pixpin:hoja=…"` detras de la ruta: el movil no reconoceria la foto y
//! la sincronizacion no encontraria el fichero (ver `md_imagen`).
//!
//! # Con que se pinta
//!
//! Con lo mismo que «Compartir como imagen» (`compartir::preparar` +
//! `generar`): el lienzo entero con sus fotos y su grafito, la pagina del
//! PDF con lo anotado encima, la tabla como rejilla, la nota como texto.
//! Un lienzo, sobre **su papel y con la tinta como la pinta el lienzo**
//! (`dibujo::tema::ordenes_como_en_el_lienzo`, lo mismo que las vistas
//! previas del chat): una pizarra se ve pizarra. A 1440 px de ancho como
//! mucho: lo que el movil decodifica de una foto de una nota
//! (`MarkdownText.cargarImagen`) y la columna del PC a doble escala.
//!
//! # Cuando se vuelve a pintar
//!
//! Al abrir la nota y luego cada dos segundos, en un hilo aparte, se mira
//! la **fecha** de lo que hace la hoja (su `.excalidraw`, el PDF, la foto y
//! su dibujo; el cuaderno si es una tabla o una nota) contra la del PNG. Si
//! la hoja es mas nueva se repinta, y la nota relee la imagen. Una tabla o
//! una nota viven en el cuaderno, que cambia con cada mensaje del chat: ahi
//! se compara ademas su texto, para no repintar por un mensaje que no es
//! suyo. Si la hoja ya no esta, la imagen se queda (es la ultima copia) y
//! la nota lleva un aviso.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_docs::md_imagen::{self, Foto};
use pixpin_notas::integracion::{GrupoDeHojas, HojaElegible, Viva};
use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};
use pixpin_store::{Catalogo, Ubicacion};

use super::{Destino, adjuntos};

/// Lo mas ancha que se guarda una pagina viva, en pixeles.
pub const LADO_VIVA: u32 = 1440;
/// Cuantos proyectos salen en «Otros proyectos» del selector: los ultimos
/// tocados. Mas seria leer decenas de cuadernos por abrir un menu.
const OTROS_PROYECTOS: usize = 15;
/// La clave de una hoja en el selector: proyecto y codigo, con un
/// separador que no sale en ninguno de los dos.
const SEP: char = '\u{1f}';

/// Una hoja de un proyecto, con el mensaje por el que se pinta y se abre.
#[derive(Debug, Clone)]
pub struct HojaDeProyecto {
    /// El id del proyecto en este equipo (su carpeta).
    pub proyecto: String,
    /// El codigo unico de la hoja (el de su mensaje).
    pub codigo: String,
    pub nombre: String,
    pub mensaje: Mensaje,
}

fn leer_proyecto_json(raiz: &Path, id: &str) -> pixpin_proyecto::Proyecto {
    std::fs::read_to_string(almacen::carpeta(raiz, id).join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Los mensajes de un proyecto, con las hojas del movil que el cuaderno
/// aun no tiene (como los lee la pantalla de Proyectos).
pub(super) fn mensajes_de(raiz: &Path, f: &Ficha) -> Vec<Mensaje> {
    let mut v = Cuaderno::leer_de(&almacen::carpeta(raiz, &f.id))
        .map(|c| c.mensajes)
        .unwrap_or_default();
    v.extend(almacen::hojas_para_ensenar(raiz, &f.id, f.aparato.as_deref().unwrap_or_default()));
    v.retain(|m| !m.en_buzon);
    v
}

/// Si un mensaje es una hoja que se puede ensenar como imagen.
fn se_pinta(m: &Mensaje) -> bool {
    match m.clase {
        // Con algo que pintar: una hoja sin dibujo ni pagina no es nada.
        Some(Clase::Dibujo) => m.referencia.as_deref().is_some_and(|r| !r.is_empty()),
        Some(Clase::Pagina) => m.pagina.is_some() || m.referencia.as_deref().is_some_and(|r| !r.is_empty()),
        Some(Clase::Imagen) => true,
        Some(Clase::MiniApp) => m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP),
        Some(Clase::Nota) => m.miniapp.is_none(),
        _ => false,
    }
}

/// El nombre de una hoja: el de `proyecto.json`, el del mensaje, el titulo
/// de la tabla o de la nota, o «Hoja N».
fn nombre_de(nombre_en_proyecto: &str, m: &Mensaje, n: usize) -> String {
    let limpio = |s: &str| s.trim().chars().take(60).collect::<String>();
    if !nombre_en_proyecto.trim().is_empty() {
        return limpio(nombre_en_proyecto);
    }
    if !m.nombre.trim().is_empty() {
        return limpio(&pixpin_docs::sin_extension(&m.nombre));
    }
    if m.clase == Some(Clase::MiniApp)
        && let Ok(t) = pixpin_proyecto::tabla::Tabla::leer(&m.texto)
        && !t.nombre.trim().is_empty()
    {
        return limpio(&t.nombre);
    }
    let titulo = pixpin_docs::md_vivo::titulo(&m.texto);
    if !titulo.is_empty() {
        return limpio(&titulo);
    }
    match m.pagina {
        Some(p) => format!("# {}", p + 1),
        None => format!("#{}", n + 1),
    }
}

/// Las hojas de un proyecto que se pueden meter en una nota, en el orden de
/// su `proyecto.json` (el de la tarjeta de Proyectos).
pub fn hojas_de(raiz: &Path, f: &Ficha) -> Vec<HojaDeProyecto> {
    let p = leer_proyecto_json(raiz, &f.id);
    let mut por_codigo: HashMap<String, Mensaje> = HashMap::new();
    for m in mensajes_de(raiz, f) {
        por_codigo.entry(m.codigo_unico()).or_insert(m);
    }
    p.hojas
        .iter()
        .enumerate()
        .filter_map(|(n, h)| {
            let m = por_codigo.remove(h.uid.as_deref()?)?;
            se_pinta(&m).then(|| HojaDeProyecto {
                proyecto: f.id.clone(),
                codigo: m.codigo_unico(),
                nombre: nombre_de(&h.nombre, &m, n),
                mensaje: m,
            })
        })
        .collect()
}

/// El proyecto de una nota, si es de uno.
pub fn proyecto_de(destino: &Destino) -> Option<&str> {
    match destino {
        Destino::Mensaje { proyecto, .. } | Destino::Nueva { proyecto } => Some(proyecto),
        Destino::Fichero { .. } => None,
    }
}

/// El codigo unico de un proyecto (el que va en los enlaces: es el mismo
/// en el movil).
pub(super) fn codigo_de_proyecto(f: &Ficha) -> String {
    pixpin_proyecto::codigos::unico(f.uid.as_deref(), "p:", &f.id)
}

/// La ficha de un proyecto por su id de aqui o por su codigo unico.
pub(super) fn ficha_de(indice: &Indice, proyecto: &str) -> Option<Ficha> {
    indice
        .buscar(proyecto)
        .or_else(|| indice.proyectos.iter().find(|f| codigo_de_proyecto(f) == proyecto))
        .cloned()
}

/// **El selector**: las hojas del proyecto de la nota y las de los ultimos
/// proyectos tocados. La nota que se esta editando no sale.
pub fn grupos(raiz: &Path, destino: &Destino) -> Vec<GrupoDeHojas> {
    let indice = Indice::leer(raiz);
    let propio = proyecto_de(destino);
    let esta = match destino {
        Destino::Mensaje { codigo, .. } => Some(codigo.as_str()),
        _ => None,
    };
    let grupo = |f: &Ficha, es_propio: bool| GrupoDeHojas {
        proyecto: f.nombre.clone(),
        propio: es_propio,
        hojas: hojas_de(raiz, f)
            .into_iter()
            .filter(|h| Some(h.codigo.as_str()) != esta)
            .map(|h| HojaElegible {
                clave: format!("{}{SEP}{}", h.proyecto, h.codigo),
                nombre: h.nombre,
            })
            .collect(),
    };
    let mut v = Vec::new();
    if let Some(f) = propio.and_then(|p| indice.buscar(p)) {
        v.push(grupo(f, true));
    }
    for f in indice
        .ordenadas()
        .into_iter()
        .filter(|f| Some(f.id.as_str()) != propio)
        .take(OTROS_PROYECTOS)
    {
        let g = grupo(f, false);
        if !g.hojas.is_empty() {
            v.push(g);
        }
    }
    v
}

/// Busca la hoja `codigo`: primero en `preferido` (el proyecto de la nota o
/// el del enlace) y si no en todos. `None` si ya no esta en ninguno.
pub fn buscar(raiz: &Path, preferido: Option<&str>, codigo: &str) -> Option<HojaDeProyecto> {
    let indice = Indice::leer(raiz);
    let en = |f: &Ficha| -> Option<HojaDeProyecto> {
        let p = leer_proyecto_json(raiz, &f.id);
        let m = mensajes_de(raiz, f)
            .into_iter()
            .find(|m| m.codigo_unico() == codigo && se_pinta(m))?;
        let (n, h) = p
            .hojas
            .iter()
            .enumerate()
            .find(|(_, h)| h.uid.as_deref() == Some(codigo))
            .map(|(n, h)| (n, h.nombre.clone()))
            .unwrap_or((0, String::new()));
        Some(HojaDeProyecto {
            proyecto: f.id.clone(),
            codigo: codigo.to_string(),
            nombre: nombre_de(&h, &m, n),
            mensaje: m,
        })
    };
    let primero = preferido.and_then(|p| ficha_de(&indice, p));
    if let Some(h) = primero.as_ref().and_then(en) {
        return Some(h);
    }
    indice
        .proyectos
        .iter()
        .filter(|f| Some(f.id.as_str()) != primero.as_ref().map(|p| p.id.as_str()))
        .find_map(en)
}

/// **Mete una hoja elegida en el selector**: el renglon de su pagina viva
/// (aun sin pintar: la pinta el vigia) o el enlace a ella.
pub fn insertar(raiz: &Path, destino: &Destino, clave: &str, como_enlace: bool) -> Option<String> {
    let (proyecto, codigo) = clave.split_once(SEP)?;
    let h = buscar(raiz, Some(proyecto), codigo)?;
    if como_enlace {
        let f = ficha_de(&Indice::leer(raiz), &h.proyecto)?;
        return Some(md_imagen::enlace_a_hoja(&h.nombre, &codigo_de_proyecto(&f), &h.codigo));
    }
    renglon_vivo(raiz, destino, &h)
}

/// El renglon de la pagina viva de `h` en la nota `destino`.
pub fn renglon_vivo(raiz: &Path, destino: &Destino, h: &HojaDeProyecto) -> Option<String> {
    let (_, ruta) = adjuntos::sitio(raiz, destino, &md_imagen::fichero_de_viva(&h.codigo))
        .inspect_err(|e| tracing::warn!(?e, "no se pudo preparar la carpeta de la pagina viva"))
        .ok()?;
    Some(md_imagen::escribir(&Foto {
        alt: h.nombre.clone(),
        ancho: None,
        ruta,
    }))
}

// ---------------------------------------------------------------------------
// Pintar

fn fecha(r: &Path) -> Option<SystemTime> {
    std::fs::metadata(r).and_then(|m| m.modified()).ok()
}

/// Si la hoja vive en el texto de su mensaje (tabla, nota): entonces su
/// fecha es la del cuaderno, que cambia con cualquier mensaje.
fn de_texto(m: &Mensaje) -> bool {
    matches!(m.clase, Some(Clase::MiniApp) | Some(Clase::Nota))
}

/// La fecha de lo ultimo que cambio de lo que hace la hoja.
pub fn fecha_de_la_hoja(raiz: &Path, h: &HojaDeProyecto) -> Option<SystemTime> {
    let carpeta = almacen::carpeta(raiz, &h.proyecto);
    let m = &h.mensaje;
    let mut v: Vec<PathBuf> = Vec::new();
    if de_texto(m) {
        v.push(carpeta.join("guardados.jsonl"));
        v.push(carpeta.join("proyecto.json"));
    }
    if let Some(r) = m.referencia.as_deref().filter(|r| !r.is_empty()) {
        v.push(almacen::lienzo(raiz, &h.proyecto, r));
    } else if m.clase == Some(Clase::Pagina) {
        // Una pagina sin dibujo: el dia que lo estrene lo dice su hoja.
        v.push(carpeta.join("proyecto.json"));
    }
    if m.pagina.is_some()
        && let Some(pdf) = crate::pdf_en_chat::documento_de(raiz, &h.proyecto)
    {
        v.push(pdf);
    }
    if let Some(r) = m.ruta.as_deref().filter(|r| !r.is_empty())
        && let Some(f) = pixpin_proyecto::vista::ruta_real(raiz, &h.proyecto, r)
    {
        let mut dibujo = f.as_os_str().to_owned();
        dibujo.push(".pixpin2d");
        v.push(PathBuf::from(dibujo));
        v.push(f);
    }
    v.iter().filter_map(|r| fecha(r)).max()
}

/// La hoja pintada como imagen, como la saca «Compartir como imagen», con
/// el papel del lienzo y a [`LADO_VIVA`] de ancho como mucho.
pub fn pintar(raiz: &Path, h: &HojaDeProyecto, t: &Catalogo) -> Result<ImagenRgba> {
    use crate::compartir::{self, Cosa};
    let mut p = compartir::preparar(
        Cosa::Mensajes {
            raiz: raiz.to_path_buf(),
            proyecto: h.proyecto.clone(),
            titulo: h.nombre.clone(),
            mensajes: vec![h.mensaje.clone()],
        },
        t,
    )?;
    let clave = match p.piezas.iter().find(|x| x.pagina.clave == h.mensaje.id) {
        Some(x) => x.pagina.clave.clone(),
        None => p.piezas.first().context("la hoja no tiene nada que pintar")?.pagina.clave.clone(),
    };
    if let Some(papel) = papel_del_lienzo(raiz, h) {
        for x in p.piezas.iter_mut().filter(|x| x.pagina.clave == clave) {
            x.fondo = papel;
            let ordenes = std::mem::take(&mut x.hoja.ordenes);
            x.hoja.ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(ordenes, papel);
        }
    }
    let carpeta = std::env::temp_dir().join("PixPin").join("vivas").join(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let fondo = p.piezas.iter().find(|x| x.pagina.clave == clave).map(|x| x.fondo);
    let salida = compartir::generar(&p, compartir::PNG, &[clave], &carpeta);
    let _ = std::fs::remove_dir_all(&carpeta);
    let img = salida?.imagen.context("sin imagen")?;
    // Con un margen de su papel: sin el, el borde de un plano toca el de la
    // imagen y en la nota parece cortado.
    let img = match fondo {
        Some(f) if p.tablas.is_empty() => con_margen(img, f),
        _ => img,
    };
    if img.ancho > LADO_VIVA {
        let alto = ((img.alto as u64 * LADO_VIVA as u64) / img.ancho as u64).max(1) as u32;
        return Ok(pixpin_codec::imagen::redimensionar(img, LADO_VIVA, alto)?);
    }
    Ok(img)
}

/// La imagen con un margen de `papel` alrededor (el 4 % del lado mayor).
fn con_margen(img: ImagenRgba, papel: pixpin_motor2d::ColorRgba) -> ImagenRgba {
    let m = (img.ancho.max(img.alto) / 25).max(8);
    let (an, al) = (img.ancho + 2 * m, img.alto + 2 * m);
    let c = [
        (papel.r * 255.0).round() as u8,
        (papel.g * 255.0).round() as u8,
        (papel.b * 255.0).round() as u8,
        255,
    ];
    let mut px = c.repeat((an * al) as usize);
    for y in 0..img.alto {
        let o = ((y * img.ancho) * 4) as usize;
        let d = (((y + m) * an + m) * 4) as usize;
        px[d..d + (img.ancho * 4) as usize].copy_from_slice(&img.pixeles[o..o + (img.ancho * 4) as usize]);
    }
    ImagenRgba {
        ancho: an,
        alto: al,
        pixeles: px,
    }
}

/// El papel de un lienzo (no de una pagina del PDF ni de una foto: esas
/// van sobre su propio fondo).
fn papel_del_lienzo(raiz: &Path, h: &HojaDeProyecto) -> Option<pixpin_motor2d::ColorRgba> {
    let m = &h.mensaje;
    if m.clase != Some(Clase::Dibujo) || m.pagina.is_some() {
        return None;
    }
    let r = m.referencia.as_deref().filter(|r| !r.is_empty())?;
    let texto = std::fs::read_to_string(almacen::lienzo(raiz, &h.proyecto, r)).ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).ok()?;
    Some(pixpin_motor2d::excalidraw::fondo(&lienzo))
}

/// Escribe el PNG de un tiron (temporal y renombrado): la nota puede estar
/// leyendolo, y el movil no puede llevarse uno a medias.
pub fn escribir_png(destino: &Path, img: &ImagenRgba) -> Result<()> {
    let png = pixpin_codec::imagen::codificar_png(img)?;
    let mut tmp = destino.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, png)?;
    std::fs::rename(&tmp, destino).with_context(|| format!("no se pudo poner {}", destino.display()))?;
    Ok(())
}

/// Un resumen del texto de una hoja de texto, para no repintarla por un
/// mensaje del chat que no es suyo.
fn huella(m: &Mensaje) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.texto.hash(&mut h);
    m.nombre.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// El vigia

#[derive(Default)]
struct Comun {
    /// Lo que cambio y aun no se le dijo a la nota.
    hechas: Vec<(String, Viva)>,
    trabajando: bool,
    /// Las que ya se dijo que no tienen hoja (para no repetirlo).
    sin_hoja: HashSet<String>,
    /// La huella del texto de cada hoja de texto la ultima vez que se pinto.
    huellas: HashMap<String, u64>,
}

/// **El vigia de las paginas vivas de una nota.** `mirar` no espera nunca:
/// recoge lo que el hilo ya hizo y, si no hay uno en marcha, lanza otro.
pub struct Vigia {
    raiz: PathBuf,
    idioma: pixpin_store::Idioma,
    comun: Arc<Mutex<Comun>>,
}

impl Vigia {
    pub fn nuevo(raiz: PathBuf, idioma: pixpin_store::Idioma) -> Vigia {
        Vigia {
            raiz,
            idioma,
            comun: Arc::default(),
        }
    }

    pub fn mirar(&self, destino: &Destino, rutas: &[String]) -> Vec<(String, Viva)> {
        let Ok(mut c) = self.comun.lock() else {
            return Vec::new();
        };
        let hechas = std::mem::take(&mut c.hechas);
        if !c.trabajando && !rutas.is_empty() {
            c.trabajando = true;
            let (raiz, destino, rutas, comun, idioma) =
                (self.raiz.clone(), destino.clone(), rutas.to_vec(), self.comun.clone(), self.idioma);
            let lanzado = std::thread::Builder::new().name("paginas-vivas".into()).spawn(move || {
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let t = Catalogo::nuevo(idioma);
                for r in &rutas {
                    let cambio = renovar(&raiz, &destino, r, &t, &comun);
                    if let (Some(v), Ok(mut c)) = (cambio, comun.lock()) {
                        c.hechas.push((r.clone(), v));
                    }
                }
                if let Ok(mut c) = comun.lock() {
                    c.trabajando = false;
                }
            });
            if lanzado.is_err() {
                c.trabajando = false;
            }
        }
        hechas
    }

    /// Lo mismo, esperando a que acabe (las pruebas).
    #[cfg(test)]
    pub fn mirar_y_esperar(&self, destino: &Destino, rutas: &[String]) -> Vec<(String, Viva)> {
        let mut todo = self.mirar(destino, rutas);
        while self.comun.lock().map(|c| c.trabajando).unwrap_or(false) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        todo.extend(self.mirar(destino, &[]));
        todo
    }
}

/// Mira una pagina viva y la repinta si hace falta. Dice lo que cambio.
fn renovar(raiz: &Path, destino: &Destino, ruta: &str, t: &Catalogo, comun: &Mutex<Comun>) -> Option<Viva> {
    let codigo = md_imagen::hoja_de_viva(ruta)?;
    let png = adjuntos::resolver(raiz, destino, ruta).or_else(|| {
        adjuntos::sitio(raiz, destino, &md_imagen::fichero_de_viva(&codigo))
            .ok()
            .map(|(p, _)| p)
    })?;
    let Some(h) = buscar(raiz, proyecto_de(destino), &codigo) else {
        let nueva = comun.lock().map(|mut c| c.sin_hoja.insert(ruta.to_string())).unwrap_or(false);
        return nueva.then_some(Viva::SinHoja);
    };
    let volvio = comun.lock().map(|mut c| c.sin_hoja.remove(ruta)).unwrap_or(false);
    let hoja = fecha_de_la_hoja(raiz, &h);
    let suya = fecha(&png);
    let vieja = match (hoja, suya) {
        (_, None) => true,
        (Some(a), Some(b)) => a > b,
        (None, Some(_)) => false,
    };
    let mut hace_falta = vieja;
    if vieja && suya.is_some() && de_texto(&h.mensaje) {
        // El cuaderno cambio, pero quiza por otro mensaje.
        let antes = comun.lock().ok().and_then(|c| c.huellas.get(&codigo).copied());
        hace_falta = antes != Some(huella(&h.mensaje));
    }
    if !hace_falta {
        return volvio.then_some(Viva::Renovada);
    }
    match pintar(raiz, &h, t).and_then(|img| escribir_png(&png, &img)) {
        Ok(()) => {
            if let Ok(mut c) = comun.lock() {
                c.huellas.insert(codigo, huella(&h.mensaje));
            }
            Some(Viva::Renovada)
        }
        Err(e) => {
            tracing::warn!(?e, %ruta, "no se pudo pintar la pagina viva");
            volvio.then_some(Viva::Renovada)
        }
    }
}

// ---------------------------------------------------------------------------
// Abrir

/// Las opciones del lienzo, que pone el arranque (`main.rs`): las hojas que
/// se abren desde una nota se dibujan con los mismos ajustes que desde el
/// chat.
static OPCIONES: Mutex<Option<crate::ventana_chat::OpcionesLienzo>> = Mutex::new(None);

pub fn poner_opciones_del_lienzo(o: crate::ventana_chat::OpcionesLienzo) {
    if let Ok(mut g) = OPCIONES.lock() {
        *g = Some(o);
    }
}

pub(super) fn opciones_del_lienzo() -> crate::ventana_chat::OpcionesLienzo {
    OPCIONES.lock().ok().and_then(|g| *g).unwrap_or(crate::ventana_chat::OpcionesLienzo {
        enganche: Default::default(),
        nivel: pixpin_nivel::Nivel::Ligero,
        medir_fotogramas: false,
    })
}

/// Que se hace al abrir algo desde una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apertura {
    /// La hoja, en su editor.
    Hoja { proyecto: String, codigo: String },
    /// Un fichero de este equipo (una foto; una pagina viva sin hoja: su
    /// ultima copia), con lo de Windows.
    Fichero(PathBuf),
    Nada,
}

/// Lo que abre `ruta` (la de una foto o un enlace `pixpin:hoja=`). Pura
/// salvo por leer el disco, para probarla sin ventanas.
pub fn que_abre(raiz: &Path, destino: &Destino, ruta: &str) -> Apertura {
    if let Some((p, c)) = md_imagen::hoja_del_enlace(ruta) {
        return match buscar(raiz, Some(&p), &c) {
            Some(h) => Apertura::Hoja {
                proyecto: h.proyecto,
                codigo: h.codigo,
            },
            None => Apertura::Nada,
        };
    }
    if let Some(c) = md_imagen::hoja_de_viva(ruta)
        && let Some(h) = buscar(raiz, proyecto_de(destino), &c)
    {
        return Apertura::Hoja {
            proyecto: h.proyecto,
            codigo: h.codigo,
        };
    }
    match adjuntos::resolver(raiz, destino, ruta).filter(|r| r.is_file()) {
        Some(r) => Apertura::Fichero(r),
        None => Apertura::Nada,
    }
}

/// Abre lo que diga [`que_abre`]: un lienzo o una pagina en el lienzo (en
/// su hilo, como el que abre el universo), una nota en su editor, y lo
/// demas (una tabla, una foto del chat) en el chat, en su mensaje.
pub fn abrir(idioma: pixpin_store::Idioma, ubicacion: &Ubicacion, destino: &Destino, ruta: &str) {
    // Un documento, un audio o un mensaje del chat (`incrustados`).
    if super::incrustados::abrir(idioma, ubicacion, destino, ruta) {
        return;
    }
    let raiz = ubicacion.raiz().to_path_buf();
    match que_abre(&raiz, destino, ruta) {
        Apertura::Fichero(r) => {
            if let Err(e) = pixpin_shell::abrir(&r) {
                tracing::warn!(?e, ruta = %r.display(), "no se pudo abrir la foto de la nota");
            }
        }
        Apertura::Nada => tracing::info!(%ruta, "nada que abrir desde la nota"),
        Apertura::Hoja { proyecto, codigo } => {
            let Some(h) = buscar(&raiz, Some(&proyecto), &codigo) else {
                return;
            };
            let m = h.mensaje;
            match m.clase {
                Some(Clase::Nota) => super::abrir(
                    idioma,
                    ubicacion.clone(),
                    Destino::Mensaje {
                        proyecto,
                        codigo,
                    },
                ),
                Some(Clase::Dibujo) | Some(Clase::Pagina) => {
                    // La referencia del dibujo, o el codigo de una pagina
                    // que aun no lo tiene (`abrir_hoja` sabe de las dos).
                    let referencia = m.referencia.clone().filter(|r| !r.is_empty()).unwrap_or(codigo);
                    let opciones = opciones_del_lienzo();
                    let lanzado = std::thread::Builder::new().name("lienzo-desde-nota".into()).spawn(move || {
                        let _com = pixpin_shell::ComDelHilo::iniciar();
                        crate::abrir_hoja::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
                        crate::ventana_chat::refrescar();
                    });
                    if let Err(e) = lanzado {
                        tracing::warn!(?e, "no se pudo lanzar el lienzo desde la nota");
                    }
                }
                _ => crate::ventana_chat::ir_a(idioma, ubicacion.clone(), opciones_del_lienzo(), proyecto, Some(codigo)),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lo que se le da al editor

/// Los textos de las fotos y las paginas vivas, del catalogo.
pub fn rotulos(t: &Catalogo) -> pixpin_notas::integracion::RotulosFotos {
    pixpin_notas::integracion::RotulosFotos {
        captura: t.t("nota-md-captura"),
        ver_grande: t.t("nota-md-ver-grande"),
        abrir_hoja: t.t("nota-md-abrir-hoja"),
        pequena: t.t("nota-md-foto-pequena"),
        mediana: t.t("nota-md-foto-mediana"),
        grande: t.t("nota-md-foto-grande"),
        columna: t.t("nota-md-foto-columna"),
        quitar: t.t("nota-md-quitar-foto"),
        hoja_borrada: t.t("nota-md-hoja-borrada"),
        otros_proyectos: t.t("nota-md-otros-proyectos"),
        sin_hojas: t.t("nota-md-sin-hojas"),
        solo_imagenes: t.t("nota-md-solo-imagenes"),
    }
}

/// **Lo que hace que la nota este integrada**: el selector de hojas, sus
/// renglones, el vigia, abrir y lo mandado desde fuera, todo sobre la nota
/// `actual` (que cambia de nueva a su mensaje al guardarse).
pub fn integracion(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    actual: &std::rc::Rc<std::cell::RefCell<Destino>>,
    inicial: &Destino,
) -> pixpin_notas::integracion::Integracion {
    let raiz = ubicacion.raiz().to_path_buf();
    let vigia = Vigia::nuevo(raiz.clone(), idioma);
    let (a1, a2, a3, a4, a5) = (actual.clone(), actual.clone(), actual.clone(), actual.clone(), actual.clone());
    let (r1, r2) = (raiz.clone(), raiz);
    let ub = ubicacion.clone();
    let inicial = inicial.clone();
    pixpin_notas::integracion::Integracion {
        hojas: Some(Box::new(move || grupos(&r1, &a1.borrow()))),
        insertar_hoja: Some(Box::new(move |clave: &str, enlace: bool| insertar(&r2, &a2.borrow(), clave, enlace))),
        vigilar: Some(Box::new(move |rutas: &[String]| vigia.mirar(&a3.borrow(), rutas))),
        abrir: Some(Box::new(move |ruta: &str| abrir(idioma, &ub, &a4.borrow(), ruta))),
        pendientes: Some(Box::new(move || recoger(&a5.borrow(), &inicial))),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Desde fuera: «Insertar en una nota»

/// Renglones mandados a una nota, hasta que esa nota los recoja.
static PENDIENTES: Mutex<Vec<(Destino, String)>> = Mutex::new(Vec::new());

/// Lo mandado a la nota `actual` (o a la que era al abrirse: una nota
/// nueva pasa a ser su mensaje al guardarse).
pub fn recoger(actual: &Destino, inicial: &Destino) -> Vec<String> {
    let Ok(mut v) = PENDIENTES.lock() else {
        return Vec::new();
    };
    let (suyos, resto): (Vec<_>, Vec<_>) = std::mem::take(&mut *v).into_iter().partition(|(d, _)| d == actual || d == inicial);
    *v = resto;
    suyos.into_iter().map(|(_, r)| r).collect()
}

/// **«Insertar en una nota»** desde el chat: la hoja del mensaje `m` como
/// pagina viva, o lo demas como su burbuja o su audio (`incrustados`), en
/// la ultima nota abierta de su proyecto o en una nota nueva del proyecto
/// si no hay ninguna abierta. `false` si no hay nada que meter.
pub fn insertar_en_nota(idioma: pixpin_store::Idioma, ubicacion: &Ubicacion, proyecto: &str, m: &Mensaje) -> bool {
    let raiz = ubicacion.raiz();
    let abierta = super::ABIERTAS.lock().ok().and_then(|v| {
        v.iter()
            .rev()
            .find(|(d, _)| proyecto_de(d) == Some(proyecto))
            .map(|(d, h)| (d.clone(), *h))
    });
    let destino = abierta.as_ref().map(|(d, _)| d.clone()).unwrap_or(Destino::Nueva {
        proyecto: proyecto.to_string(),
    });
    // Una hoja que se pinta, como su pagina viva; lo demas del chat (un
    // archivo, una nota de voz, un texto), como su burbuja o su audio
    // (`incrustados`).
    let viva = if se_pinta(m) && m.clase != Some(Clase::Nota) {
        buscar(raiz, Some(proyecto), &m.codigo_unico()).and_then(|h| renglon_vivo(raiz, &destino, &h))
    } else {
        None
    };
    let renglon = viva.or_else(|| {
        let f = ficha_de(&Indice::leer(raiz), proyecto).filter(|_| super::incrustados::se_puede_enlazar(m))?;
        Some(super::incrustados::bloque_de(raiz, &f, m, &Catalogo::nuevo(idioma)))
    });
    let Some(renglon) = renglon else {
        return false;
    };
    if let Ok(mut v) = PENDIENTES.lock() {
        v.push((destino.clone(), renglon));
    }
    match abierta {
        Some((_, hwnd)) => {
            // Si ya no existe, lo pendiente se recoge al abrirla otra vez.
            pixpin_notas::integracion::tocar(hwnd);
            pixpin_shell::overlay::VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(hwnd as *mut _));
        }
        None => super::abrir(idioma, ubicacion.clone(), destino),
    }
    true
}

/// Si el chat tiene que ofrecer «Insertar en una nota» para ese mensaje:
/// las hojas que se dibujan (lienzo, pagina, foto) y las tablas, como su
/// pagina viva; y desde el 1-oct todo lo demas del chat (un archivo, una
/// nota de voz, un texto) como su burbuja o su audio (`incrustados`).
pub fn se_puede_insertar(m: &Mensaje) -> bool {
    (se_pinta(m) && m.clase != Some(Clase::Nota)) || super::incrustados::se_puede_enlazar(m)
}

#[cfg(test)]
mod pruebas;
