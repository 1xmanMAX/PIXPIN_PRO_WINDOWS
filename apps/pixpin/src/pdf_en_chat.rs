//! **Un PDF en el chat de un proyecto**, como en el movil
//! (`guardados/UnirAlProyecto.kt`).
//!
//! Soltar o pegar un PDF en la conversacion lo deja como UN mensaje de
//! archivo, con la primera pagina de vista previa, y nada mas: el usuario vio
//! que al entrar se extraian todas las paginas como imagenes y que eso daba
//! tirones («solo se extraigan si le doy a anadir a proyectos»). Extraer es
//! decision suya, desde el menu del mensaje:
//!
//! - **«Anadir al proyecto»** hace lo del movil: el PDF pasa a ser el
//!   documento del proyecto (`pdfOrigen`, una copia `doc-<t>.pdf`, y su copia
//!   limpia) y cada pagina una hoja `pagina` que se pinta DESDE el PDF. No se
//!   guarda ninguna imagen: cada vista pide la pagina al tamano al que se va
//!   a ver, y el lector la repinta al ampliar, asi que siempre se ve nitida.
//!   Si el proyecto ya tenia documento, las paginas se pegan detras
//!   ([`pixpin_pdf::union`]); si no se dejan (un PDF cifrado), se pintan y
//!   se pega lo pintado como PDF propio; y si ni eso, fotos clavadas.
//! - **«Anadir como imagenes»**: cada pagina (hasta [`TOPE_DE_PAGINAS_COMO_FOTO`])
//!   como una foto clavada en su lienzo, que es la salida del movil para lo
//!   que no se deja pegar.
//!
//! **Nada de esto corre en el hilo de la ventana.** Pintar una pagina son
//! decenas de milisegundos, y un documento de cien paginas, segundos. Las
//! vistas previas las pinta un hilo propio y las deja en una cache en disco;
//! unir y pasar a imagenes van en otro, con su progreso, y la ventana solo
//! recoge lo terminado.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

use pixpin_proyecto::{Hoja, Proyecto, almacen, cuaderno};

pub use crate::fondo_lienzo::ANCHO_PAPEL_PDF;

/// Ancho al que se guarda una pagina como foto: legible sin pesar decenas de
/// megas (`ANCHO_DE_PAGINA` del movil). **No** es la medida de la pagina en
/// un lienzo dibujado encima: esa es [`ANCHO_PAPEL_PDF`] (1400), la del
/// `PdfDoc.PAGE_WIDTH` del movil. Con 1600 lo dibujado en el movil caia
/// corrido un 14 % en el PC.
pub const ANCHO_PAGINA: u32 = 1600;
/// Cuantas paginas se meten como fotos, como mucho (`TOPE_DE_PAGINAS_COMO_FOTO`).
pub const TOPE_DE_PAGINAS_COMO_FOTO: u32 = 40;
/// Cuantas hojas puede tener un proyecto (`Proyectos.MAX_HOJAS`).
pub const MAX_HOJAS: usize = 200;
/// Ancho de las vistas previas de una pagina: la burbuja y la galeria no
/// pasan de 260 px, y al 200 % de escala esto sigue sobrando.
pub const ANCHO_VISTA: u32 = 640;

// ---------------------------------------------------------------------------
// Las paginas pintadas para verse, en su hilo

struct Cola {
    pedidos: VecDeque<(PathBuf, u32, u32, PathBuf)>,
    en_cola: HashSet<PathBuf>,
    fallidas: HashSet<PathBuf>,
}

fn cola() -> &'static (Mutex<Cola>, Condvar) {
    static COLA: OnceLock<(Mutex<Cola>, Condvar)> = OnceLock::new();
    COLA.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("pdf-paginas".into())
            .spawn(hilo_de_paginas);
        (
            Mutex::new(Cola {
                pedidos: VecDeque::new(),
                en_cola: HashSet::new(),
                fallidas: HashSet::new(),
            }),
            Condvar::new(),
        )
    })
}

/// Hay paginas pintadas que la ventana aun no ha recogido.
static NOVEDADES: AtomicBool = AtomicBool::new(false);
/// La ventana a la que despertar cuando llega algo (0: ninguna).
static VENTANA: AtomicIsize = AtomicIsize::new(0);

/// A quien despertar cuando haya paginas o trabajos terminados.
pub fn avisar_a(hwnd: isize) {
    VENTANA.store(hwnd, Ordering::Relaxed);
}

pub(crate) fn despertar() {
    let h = VENTANA.load(Ordering::Relaxed);
    if h != 0 {
        pixpin_shell::overlay::despertar(h);
    }
}

/// Si han llegado paginas desde la ultima vez que se pregunto.
pub fn hay_paginas_nuevas() -> bool {
    NOVEDADES.swap(false, Ordering::AcqRel)
}

/// Donde se guarda la pagina `pagina` de `pdf` pintada a `ancho`.
///
/// Fuera de la carpeta del proyecto a proposito: todo lo que hay en ella
/// viaja al empaquetar, y una cache no es trabajo del usuario. La clave
/// lleva el tamano y la fecha del PDF: si se le pegan paginas o se aligera,
/// lo pintado antes no se confunde con lo de ahora.
fn en_cache(raiz: &Path, pdf: &Path, pagina: u32, ancho: u32) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(pdf).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    pdf.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    Some(
        raiz.join("cache")
            .join("paginas-pdf")
            .join(format!("{:016x}-p{pagina}-w{ancho}.png", h.finish())),
    )
}

/// **La pagina ya pintada, o `None` mientras se pinta.** Nunca pinta aqui:
/// si no esta, se pide al hilo y se vuelve enseguida; cuando llegue,
/// [`hay_paginas_nuevas`] lo dice. Devuelve la ruta del PNG y lo que mide la
/// pagina en unidades de lienzo (ancho [`ANCHO_PAPEL_PDF`], el del movil), que
/// es donde caen los trazos que se hayan dibujado encima.
pub fn pagina_pintada(
    raiz: &Path,
    pdf: &Path,
    pagina: u32,
    ancho: u32,
) -> Option<(PathBuf, f32, f32)> {
    let destino = en_cache(raiz, pdf, pagina, ancho)?;
    if let Ok((w, h)) = pixpin_codec::imagen::medidas(&destino)
        && w > 0
    {
        let unidades = ANCHO_PAPEL_PDF;
        return Some((destino, unidades, unidades * h as f32 / w as f32));
    }
    let (m, cv) = cola();
    if let Ok(mut c) = m.lock()
        && !c.fallidas.contains(&destino)
        && c.en_cola.insert(destino.clone())
    {
        c.pedidos
            .push_back((pdf.to_path_buf(), pagina, ancho, destino));
        cv.notify_one();
    }
    None
}

fn hilo_de_paginas() {
    // Abrir un PDF es lo caro; pintar una pagina, no. Se guarda el ultimo.
    let mut abierto: Option<(PathBuf, pixpin_pdf::Documento)> = None;
    loop {
        let (m, cv) = cola();
        let pedido = {
            let Ok(mut c) = m.lock() else { return };
            loop {
                if let Some(p) = c.pedidos.pop_front() {
                    break p;
                }
                c = match cv.wait(c) {
                    Ok(c) => c,
                    Err(_) => return,
                };
            }
        };
        let (pdf, pagina, ancho, destino) = pedido;
        if abierto.as_ref().is_none_or(|(r, _)| *r != pdf) {
            abierto = pixpin_pdf::Documento::abrir(&pdf)
                .inspect_err(
                    |e| tracing::info!(?e, pdf = %pdf.display(), "PDF que no se abre para pintar"),
                )
                .ok()
                .map(|d| (pdf.clone(), d));
        }
        let pintar = |d: &pixpin_pdf::Documento| {
            let img = d.renderizar(pagina, ancho).ok()?;
            let png = pixpin_codec::codificar_png(&img).ok()?;
            std::fs::create_dir_all(destino.parent()?).ok()?;
            // Al lado y luego el nombre: quien mira la cache no puede ver
            // un PNG a medio escribir.
            let temporal = destino.with_extension("png.tmp");
            std::fs::write(&temporal, png).ok()?;
            std::fs::rename(&temporal, &destino).ok()
        };
        let mut hecho = abierto.as_ref().and_then(|(_, d)| pintar(d));
        // **La pagina, aunque el documento este roto** (E7, `paginaSana` del
        // movil): se rehace desde la copia limpia y se vuelve a intentar una
        // vez; si ni asi, se pinta la copia.
        if hecho.is_none()
            && let Some(otra) = crate::pdf_del_proyecto::reparar(&pdf)
        {
            abierto = pixpin_pdf::Documento::abrir(&otra)
                .ok()
                .map(|d| (otra.clone(), d));
            hecho = abierto.as_ref().and_then(|(_, d)| pintar(d));
        }
        if let Ok(mut c) = m.lock() {
            c.en_cola.remove(&destino);
            if hecho.is_none() {
                c.fallidas.insert(destino);
            }
        }
        NOVEDADES.store(true, Ordering::Release);
        despertar();
    }
}

// ---------------------------------------------------------------------------
// Unir y pasar a imagenes, en su hilo

/// Que se pidio hacer con el PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trabajo {
    Unir,
    ComoImagenes,
}

/// Como acabaron las paginas en el proyecto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Como {
    /// El PDF es ahora el documento del proyecto.
    Documento,
    /// Sus paginas, pegadas detras del documento que ya habia.
    Pegadas,
    /// Pintadas y pegadas como PDF propio (no se dejaba pegar tal cual).
    Pintadas,
    /// Fotos clavadas, cada una en su lienzo.
    Fotos,
}

#[derive(Debug)]
pub struct Unido {
    pub hojas: usize,
    pub como: Como,
    /// Cuantas paginas tenia el PDF: con fotos solo entran las primeras.
    pub paginas: u32,
}

/// Un trabajo acabado, para que la ventana lo apunte y lo diga.
pub struct Terminado {
    pub ficha: String,
    pub mensaje: String,
    pub resultado: Result<Unido, String>,
}

#[derive(Default)]
struct Trabajos {
    en_curso: HashMap<(String, String), (Trabajo, u32, u32)>,
    terminados: Vec<Terminado>,
}

fn trabajos() -> &'static Mutex<Trabajos> {
    static T: OnceLock<Mutex<Trabajos>> = OnceLock::new();
    T.get_or_init(Default::default)
}

/// Si ese mensaje ya tiene un trabajo en marcha: pulsar dos veces no lo une
/// dos veces.
pub fn ocupado(ficha: &str, mensaje: &str) -> bool {
    trabajos().lock().is_ok_and(|t| {
        t.en_curso
            .contains_key(&(ficha.to_string(), mensaje.to_string()))
    })
}

/// El progreso del trabajo que va, si va alguno: que es, cuantas y de cuantas.
pub fn progreso() -> Option<(Trabajo, u32, u32)> {
    trabajos().lock().ok()?.en_curso.values().next().copied()
}

/// Lo que ha terminado desde la ultima vez.
pub fn terminados() -> Vec<Terminado> {
    trabajos()
        .lock()
        .map(|mut t| std::mem::take(&mut t.terminados))
        .unwrap_or_default()
}

/// Empieza a unir (o a pasar a imagenes) el PDF de un mensaje, en su hilo.
/// `false` si ya habia uno con ese mensaje o no se pudo lanzar.
pub fn empezar(
    trabajo: Trabajo,
    raiz: &Path,
    ficha: &almacen::Ficha,
    mensaje: &str,
    pdf: &Path,
    nombre: &str,
) -> bool {
    let clave = (ficha.id.clone(), mensaje.to_string());
    {
        let Ok(mut t) = trabajos().lock() else {
            return false;
        };
        if t.en_curso.contains_key(&clave) {
            return false;
        }
        t.en_curso.insert(clave.clone(), (trabajo, 0, 0));
    }
    let (raiz, ficha, mensaje, pdf, nombre) = (
        raiz.to_path_buf(),
        ficha.clone(),
        mensaje.to_string(),
        pdf.to_path_buf(),
        nombre.to_string(),
    );
    let c2 = clave.clone();
    let lanzado = std::thread::Builder::new()
        .name("pdf-unir".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let avance = |hechas: u32, total: u32| {
                if let Ok(mut t) = trabajos().lock()
                    && let Some(e) = t.en_curso.get_mut(&c2)
                {
                    *e = (trabajo, hechas, total);
                }
                despertar();
            };
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            // Un PowerPoint entra como su PDF (D10, `DiapositivasAPdf` del
            // movil): se convierte aqui, en este mismo hilo, y lo demas no
            // se entera. Un PDF pasa tal cual.
            let resultado =
                crate::diapositivas::pdf_para_unir(&raiz, &pdf).and_then(|pdf| match trabajo {
                    Trabajo::Unir => unir(&raiz, &ficha, &pdf, &mensaje, &nombre, ahora, &avance),
                    Trabajo::ComoImagenes => {
                        como_imagenes(&raiz, &ficha, &pdf, &mensaje, &nombre, ahora, &avance)
                    }
                });
            if let Err(e) = &resultado {
                tracing::warn!(%e, pdf = %pdf.display(), ?trabajo, "no se pudo unir el PDF");
            }
            if let Ok(mut t) = trabajos().lock() {
                t.en_curso.remove(&c2);
                t.terminados.push(Terminado {
                    ficha: ficha.id.clone(),
                    mensaje,
                    resultado,
                });
            }
            despertar();
        });
    if lanzado.is_err() {
        if let Ok(mut t) = trabajos().lock() {
            t.en_curso.remove(&clave);
        }
        return false;
    }
    true
}

fn leer_proyecto(raiz: &Path, ficha: &almacen::Ficha) -> Proyecto {
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let mut p: Proyecto = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // Uno nacido aqui no lo tiene todavia: se empieza con lo de la ficha,
    // como hace «Unir» con una foto.
    if p.id.is_empty() {
        p.id = ficha.id.clone();
        p.nombre = ficha.nombre.clone();
    }
    p
}

fn guardar_proyecto(raiz: &Path, ficha: &str, p: &Proyecto) -> Result<(), String> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let texto = serde_json::to_string(p).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    // Al lado y luego de un tiron: un corte a mitad no deja el proyecto sin
    // sus hojas.
    let temporal = carpeta.join("proyecto.json.pdf.tmp");
    std::fs::write(&temporal, texto).map_err(|e| e.to_string())?;
    std::fs::rename(&temporal, carpeta.join("proyecto.json")).map_err(|e| e.to_string())
}

fn hoja(id: String, nombre: String, de_mensaje: &str) -> Hoja {
    let mut h = Hoja {
        id,
        nombre,
        ..Default::default()
    };
    // Cada hoja se queda con la sena de su mensaje (`Hoja.deMensaje`): es lo
    // que dice que salio de el, lo que esconde su vista del chat y lo que
    // permite «Volver a anadir» si se quita.
    h.resto.insert(
        "deMensaje".into(),
        serde_json::Value::String(de_mensaje.to_string()),
    );
    h.uid = Some(h.codigo_unico());
    h
}

/// Cuantas paginas tiene: leyendo el arbol (barato) y, si no se entiende,
/// preguntando a Windows.
fn paginas_de(bytes: &[u8], ruta: &Path) -> Option<u32> {
    pixpin_pdf::union::contar_paginas(bytes)
        .or_else(|| pixpin_pdf::Documento::abrir(ruta).ok().map(|d| d.paginas()))
}

/// **`UnirAlProyecto.hojasDelPdf`**: el PDF como documento del proyecto o,
/// si ya tenia uno, sus paginas detras. Corre en el hilo de los trabajos.
pub fn unir(
    raiz: &Path,
    ficha: &almacen::Ficha,
    pdf: &Path,
    de_mensaje: &str,
    nombre: &str,
    ahora: i64,
    avance: &dyn Fn(u32, u32),
) -> Result<Unido, String> {
    // Si el PDF aun se esta aligerando (entro hace nada), se espera: el
    // proyecto tiene que nacer con la version ligera, tambien cuando sus
    // paginas se pegan detras de otro documento.
    crate::aligerar::esperar(pdf);
    let bytes = std::fs::read(pdf).map_err(|e| e.to_string())?;
    let paginas = paginas_de(&bytes, pdf).ok_or("el PDF no se deja leer")?;
    let mut p = leer_proyecto(raiz, ficha);
    let sitio = MAX_HOJAS.saturating_sub(p.hojas.len());
    match pixpin_proyecto::vista::documento_del_proyecto(raiz, &ficha.id) {
        None => {
            // Se copia antes de nada: el adjunto del chat se borra con su
            // mensaje, y el proyecto no puede apuntar a lo que ya no esta.
            let carpeta = almacen::carpeta(raiz, &ficha.id).join("archivos");
            std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
            let origen = carpeta.join(format!("doc-{ahora}.pdf"));
            std::fs::write(&origen, &bytes).map_err(|e| e.to_string())?;
            // **No se aligera aqui**: el adjunto ya se aligero al entrar (o se
            // esta aligerando, y arriba se espero a que acabara), y esto es
            // copia suya. Si es de antes de aligerar al entrar, lo hace la
            // cola despues —documento y copia limpia, que son gemelos—, sin
            // parar este hilo. Ver `crate::aligerar`.
            let limpio = carpeta.join(format!("limpio-{ahora}.pdf"));
            let limpio = std::fs::copy(&origen, &limpio).ok().map(|_| limpio);
            crate::aligerar::tras_unir(raiz, &origen);
            let cuantas = (paginas as usize).min(sitio);
            p.hojas.extend((0..cuantas).map(|i| {
                let mut h = hoja(format!("h-{ahora}-{i}"), String::new(), de_mensaje);
                h.pagina = Some(i as u32);
                h
            }));
            p.pdf_origen = Some(format!("archivos/doc-{ahora}.pdf"));
            p.pdf_limpio = limpio.map(|_| format!("archivos/limpio-{ahora}.pdf"));
            p.tocado = ahora;
            guardar_proyecto(raiz, &ficha.id, &p)?;
            Ok(Unido {
                hojas: cuantas,
                como: Como::Documento,
                paginas,
            })
        }
        Some(documento) => {
            // **Con documento, las paginas nuevas se pegan al documento**:
            // asi son paginas de verdad —estaticas, vectoriales— y no fotos
            // movibles. Solo si no se puede se pintan, y si ni eso, fotos.
            let limpio = p
                .pdf_limpio
                .as_deref()
                .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, r))
                .filter(|r| r.is_file());
            let antes_bytes = std::fs::read(&documento).map_err(|e| e.to_string())?;
            let antes = paginas_de(&antes_bytes, &documento).ok_or("el documento no se lee")?;
            let (pegar, como) = match pixpin_pdf::union::anadir_paginas(&antes_bytes, &bytes) {
                Some(unidos) => (Some((unidos, bytes.clone())), Como::Pegadas),
                None => {
                    let pintado = pintar_paginas(pdf, paginas, avance)
                        .and_then(|imgs| pixpin_pdf::union::de_imagenes(&imgs));
                    let unidos = pintado.as_ref().and_then(|propio| {
                        Some((
                            pixpin_pdf::union::anadir_paginas(&antes_bytes, propio)?,
                            propio.clone(),
                        ))
                    });
                    (unidos, Como::Pintadas)
                }
            };
            let Some((unidos, pegado)) = pegar else {
                return como_imagenes(raiz, ficha, pdf, de_mensaje, nombre, ahora, avance);
            };
            // La copia limpia primero: si algo falla a medias, el original
            // sigue entero.
            if let Some(l) = &limpio
                && let Some(u) = std::fs::read(l)
                    .ok()
                    .and_then(|b| pixpin_pdf::union::anadir_paginas(&b, &pegado))
            {
                let _ = escribir_atomico(l, &u);
            }
            escribir_atomico(&documento, &unidos).map_err(|e| e.to_string())?;
            let despues = pixpin_pdf::union::contar_paginas(&unidos).unwrap_or(antes);
            let nuevas = (despues.saturating_sub(antes) as usize).min(sitio);
            p.hojas.extend((0..nuevas).map(|i| {
                let n = antes + i as u32;
                let mut h = hoja(format!("h-{ahora}-{n}"), String::new(), de_mensaje);
                h.pagina = Some(n);
                h
            }));
            p.tocado = ahora;
            guardar_proyecto(raiz, &ficha.id, &p)?;
            Ok(Unido {
                hojas: nuevas,
                como,
                paginas,
            })
        }
    }
}

fn escribir_atomico(ruta: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporal = ruta.with_extension("pdf.nuevo");
    std::fs::write(&temporal, bytes)?;
    std::fs::rename(&temporal, ruta)
}

/// Las primeras paginas pintadas a [`ANCHO_PAGINA`], con su avance.
fn pintar_paginas(
    pdf: &Path,
    paginas: u32,
    avance: &dyn Fn(u32, u32),
) -> Option<Vec<pixpin_codec::imagen::ImagenRgba>> {
    let d = pixpin_pdf::Documento::abrir(pdf).ok()?;
    let cuantas = paginas.min(d.paginas()).min(TOPE_DE_PAGINAS_COMO_FOTO);
    let mut salida = Vec::new();
    for i in 0..cuantas {
        avance(i, cuantas);
        match d.renderizar(i, ANCHO_PAGINA) {
            Ok(img) => salida.push(img),
            Err(e) => tracing::warn!(?e, pagina = i, "pagina que no se pudo pintar"),
        }
    }
    (!salida.is_empty()).then_some(salida)
}

/// **Cada pagina como una foto clavada en su lienzo** (`hojaConFoto` con
/// `clavada = true` del movil): hasta [`TOPE_DE_PAGINAS_COMO_FOTO`], a
/// [`ANCHO_PAGINA`]. Corre en el hilo de los trabajos.
pub fn como_imagenes(
    raiz: &Path,
    ficha: &almacen::Ficha,
    pdf: &Path,
    de_mensaje: &str,
    nombre: &str,
    ahora: i64,
    avance: &dyn Fn(u32, u32),
) -> Result<Unido, String> {
    let d = pixpin_pdf::Documento::abrir(pdf).map_err(|e| e.to_string())?;
    let paginas = d.paginas();
    let mut p = leer_proyecto(raiz, ficha);
    let sitio = MAX_HOJAS.saturating_sub(p.hojas.len()) as u32;
    let cuantas = paginas.min(TOPE_DE_PAGINAS_COMO_FOTO).min(sitio);
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(carpeta.join("imagenes")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(carpeta.join("lienzos")).map_err(|e| e.to_string())?;
    let mut hechas = 0;
    for i in 0..cuantas {
        avance(i, cuantas);
        let hecho = (|| -> Result<Hoja, String> {
            let img = d.renderizar(i, ANCHO_PAGINA).map_err(|e| e.to_string())?;
            let png = pixpin_codec::codificar_png(&img).map_err(|e| e.to_string())?;
            let foto = format!("pag{ahora}x{i}");
            std::fs::write(carpeta.join("imagenes").join(&foto), &png)
                .map_err(|e| e.to_string())?;
            let dibujo = format!("dib-{ahora}-{i}");
            let texto = lienzo_con_foto(&foto, img.ancho, img.alto, ahora, i);
            std::fs::write(almacen::lienzo(raiz, &ficha.id, &dibujo), texto)
                .map_err(|e| e.to_string())?;
            let mut h = hoja(
                format!("hoja-{ahora}-{i}"),
                format!("{nombre} {}", i + 1),
                de_mensaje,
            );
            h.dibujo = Some(dibujo);
            Ok(h)
        })();
        match hecho {
            Ok(h) => {
                p.hojas.push(h);
                hechas += 1;
            }
            Err(e) => tracing::warn!(%e, pagina = i, "pagina que no se pudo pasar a foto"),
        }
    }
    avance(cuantas, cuantas);
    if hechas == 0 {
        return Err("no salio ninguna pagina".into());
    }
    p.tocado = ahora;
    guardar_proyecto(raiz, &ficha.id, &p)?;
    Ok(Unido {
        hojas: hechas,
        como: Como::Fotos,
        paginas,
    })
}

/// El `.excalidraw` de una foto clavada: la foto a lo ancho de una hoja
/// (1400, como el movil), al fondo y bloqueada para que no se mueva al
/// dibujar encima. La foto va por ruta (`imagenes/<id>`), no en base64.
fn lienzo_con_foto(foto: &str, ancho: u32, alto: u32, ahora: i64, i: u32) -> String {
    let escala = (1400.0 / ancho.max(1) as f64).min(1.0);
    serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-max",
        "elements": [{
            "id": format!("foto-{ahora}-{i}"),
            "type": "image",
            "x": 0, "y": 0,
            "width": ancho as f64 * escala,
            "height": alto as f64 * escala,
            "angle": 0,
            "strokeColor": "transparent",
            "backgroundColor": "transparent",
            "fillStyle": "solid",
            "strokeWidth": 1,
            "strokeStyle": "solid",
            "roughness": 0,
            "opacity": 100,
            "groupIds": [],
            "seed": 1,
            "version": 1,
            "versionNonce": 1,
            "isDeleted": false,
            "boundElements": null,
            "locked": true,
            "fileId": foto,
            "status": "saved",
            "scale": [1, 1]
        }],
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {
            foto: {"id": foto, "mimeType": "image/png", "path": format!("imagenes/{foto}"), "created": ahora}
        }
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Lo que ensena el chat

/// El documento del proyecto ([`pixpin_proyecto::vista::documento_del_proyecto`])
/// recordado mientras su `proyecto.json` no cambie: cada hoja de PDF lo
/// pregunta al leer su vista, y con una ruta del movil encontrarlo es leer
/// la lista de proyectos entera. Cien paginas no pueden ser cien vueltas.
pub fn documento_de(raiz: &Path, ficha: &str) -> Option<PathBuf> {
    type Recuerdo = HashMap<(PathBuf, String), (Option<std::time::SystemTime>, Option<PathBuf>)>;
    thread_local! {
        static CACHE: std::cell::RefCell<Recuerdo> = std::cell::RefCell::new(HashMap::new());
    }
    let fecha = std::fs::metadata(almacen::carpeta(raiz, ficha).join("proyecto.json"))
        .and_then(|m| m.modified())
        .ok();
    let clave = (raiz.to_path_buf(), ficha.to_string());
    if let Some(ruta) = CACHE.with_borrow(|c| {
        c.get(&clave)
            .filter(|(f, r)| *f == fecha && r.as_ref().is_none_or(|r| r.is_file()))
            .map(|(_, r)| r.clone())
    }) {
        return ruta;
    }
    let ruta = pixpin_proyecto::vista::documento_del_proyecto(raiz, ficha);
    // Quien pinta la pagina solo tiene la ruta: se le deja dicho donde esta
    // la copia limpia por si el documento esta roto (E7).
    if let Some(r) = &ruta {
        crate::pdf_del_proyecto::apuntar(r, crate::pdf_del_proyecto::limpio_de(raiz, ficha));
    }
    CACHE.with_borrow_mut(|c| c.insert(clave, (fecha, ruta.clone())));
    ruta
}

/// Si es un PDF, por el nombre o la ruta del mensaje.
pub fn es_pdf(m: &cuaderno::Mensaje) -> bool {
    let fin = |s: &str| s.to_lowercase().ends_with(".pdf");
    fin(&m.nombre) || m.ruta.as_deref().is_some_and(fin)
}

/// Marca como «solo para la galeria» las hojas de ensenar (desde
/// `desde` en `mensajes`) que salieron de un mensaje que esta en este mismo
/// chat: las paginas de un PDF unido, sus fotos. El mensaje del PDF ya las
/// representa en la conversacion; repetirlas debajo, cien burbujas de
/// paginas, era lo que el usuario no queria («que no implique que se anadan
/// las imagenes al chat»). En la galeria del proyecto siguen todas.
///
/// Y, en TODO el chat (tambien lo que ya esta escrito en el cuaderno), las
/// **paginas del PDF** del proyecto, como el movil
/// (`almacen::paginas_fuera_del_chat`): del PDF el chat solo lleva su
/// mensaje, venga de este chat, de otro o del movil. Los cuadernos viejos
/// tienen escrito un mensaje por pagina (el PC los ponia al importar y al
/// completar hojas); no se borran —el cuaderno viaja al movil y alli son
/// datos de otro— solo no se ensenan. La excepcion es la pagina que el
/// usuario mando el mismo al chat y luego unio: ese mensaje es el origen de
/// su hoja (`deMensaje`) y se queda.
pub fn marcar_solo_galeria(carpeta: &Path, mensajes: &mut [cuaderno::Mensaje], desde: usize) {
    let Some(p) = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Proyecto>(&t).ok())
    else {
        return;
    };
    let paginas = almacen::paginas_fuera_del_chat(&p);
    for m in mensajes.iter_mut() {
        let de_una_pagina = m
            .uid
            .as_deref()
            .and_then(|u| paginas.get(u))
            .is_some_and(|origen| origen.as_deref() != Some(m.id.as_str()));
        if de_una_pagina {
            m.resto
                .insert(SOLO_GALERIA.into(), serde_json::Value::Bool(true));
        }
    }
    if desde >= mensajes.len() {
        return;
    }
    let ids: HashSet<&str> = mensajes[..desde].iter().map(|m| m.id.as_str()).collect();
    let de_un_mensaje: HashSet<String> = p
        .hojas
        .iter()
        .filter(|h| {
            h.resto
                .get("deMensaje")
                .and_then(|v| v.as_str())
                .is_some_and(|d| ids.contains(d))
        })
        .filter_map(|h| h.uid.clone())
        .collect();
    for m in &mut mensajes[desde..] {
        if m.uid.as_ref().is_some_and(|u| de_un_mensaje.contains(u)) {
            m.resto
                .insert(SOLO_GALERIA.into(), serde_json::Value::Bool(true));
        }
    }
}

/// **Lo que el chat de un proyecto tiene que saber, ademas de su cuaderno**:
/// suma a `mensajes` (lo leido del cuaderno) las hojas del `proyecto.json`
/// que el cuaderno no tiene, SOLO para verlas, y marca las que van solo a la
/// galeria ([`marcar_solo_galeria`]). Se marca siempre, aunque no falte
/// ninguna hoja: las paginas viejas escritas en el cuaderno tambien se
/// esconden.
pub fn para_el_chat(raiz: &Path, id: &str, aparato: &str, mensajes: &mut Vec<cuaderno::Mensaje>) {
    let desde = mensajes.len();
    mensajes.extend(almacen::hojas_para_ensenar(raiz, id, aparato));
    if mensajes.len() > desde {
        tracing::info!(cuantas = mensajes.len() - desde, proyecto = %id, "hojas del proyecto que solo se ensenan");
    }
    marcar_solo_galeria(&almacen::carpeta(raiz, id), mensajes, desde);
}

/// La marca de [`marcar_solo_galeria`]. Con guion bajo: no es del formato,
/// y si alguna vez se escribiera, kotlinx la ignora al normalizar.
pub const SOLO_GALERIA: &str = "_soloGaleria";

/// Si un mensaje solo se ensena en la galeria (ver [`marcar_solo_galeria`]).
pub fn solo_galeria(m: &cuaderno::Mensaje) -> bool {
    m.resto.get(SOLO_GALERIA).and_then(|v| v.as_bool()) == Some(true)
}

/// **Lo que dice la cabecera del chat**: cuantos mensajes se ven y cuanto
/// ocupan. Sin las paginas escondidas: «8 cosas» con dos burbujas a la vista
/// parecia que faltaban seis.
pub fn cuenta_de_la_cabecera(mensajes: &[cuaderno::Mensaje]) -> (usize, i64) {
    mensajes
        .iter()
        .filter(|m| !solo_galeria(m))
        .fold((0, 0), |(n, b), m| (n + 1, b + m.bytes.max(0)))
}

/// **El punto rojo del movil**: el mensaje se unio al proyecto y su hoja ya
/// no esta (se quito, o la quito la sincronizacion). Lee `proyecto.json`
/// solo cuando cambia: se pregunta en cada fotograma.
// La cache interna es de un solo uso; un alias no aclara nada.
#[allow(clippy::type_complexity)]
pub fn sin_su_hoja(carpeta: &Path, m: &cuaderno::Mensaje) -> bool {
    if m.resto.get("unido").and_then(|v| v.as_bool()) != Some(true) {
        return false;
    }
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(PathBuf, Option<std::time::SystemTime>, HashSet<String>)>> =
            const { std::cell::RefCell::new(None) };
    }
    let ruta = carpeta.join("proyecto.json");
    let fecha = std::fs::metadata(&ruta).and_then(|m| m.modified()).ok();
    CACHE.with_borrow_mut(|c| {
        let vale = c
            .as_ref()
            .is_some_and(|(r, f, _)| *r == ruta && *f == fecha);
        if !vale {
            let senas = std::fs::read_to_string(&ruta)
                .ok()
                .and_then(|t| serde_json::from_str::<Proyecto>(&t).ok())
                .map(|p| {
                    p.hojas
                        .iter()
                        .flat_map(|h| {
                            [
                                Some(h.id.clone()),
                                h.resto
                                    .get("deMensaje")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                // Y el dibujo: un lienzo del movil tiene su hoja
                                // aunque el id del mensaje no coincida (salia
                                // rojo sin motivo, 3-oct-2026).
                                h.dibujo.as_ref().map(|d| format!("dibujo:{d}")),
                            ]
                        })
                        .flatten()
                        .collect()
                })
                .unwrap_or_default();
            *c = Some((ruta.clone(), fecha, senas));
        }
        c.as_ref().is_some_and(|(_, _, s)| {
            !s.contains(&m.id)
                && m.referencia
                    .as_ref()
                    .is_none_or(|r| !s.contains(&format!("dibujo:{r}")))
        })
    })
}

// ---------------------------------------------------------------------------
// Abrir una hoja-pagina: el lienzo con la pagina de fondo, como en el movil

/// Si un mensaje es una hoja-pagina del documento sin dibujo propio todavia:
/// la que al abrirse necesita que se le cree el suyo. Una pagina que se
/// extrajo como PNG (`ruta` puesta, de versiones viejas) no lo es.
pub fn es_pagina_sin_dibujo(m: &cuaderno::Mensaje) -> bool {
    m.clase == Some(cuaderno::Clase::Pagina)
        && m.pagina.is_some()
        && m.ruta.as_deref().is_none_or(str::is_empty)
        && m.referencia.as_deref().is_none_or(str::is_empty)
}

/// La hoja del `proyecto.json` que representa ese mensaje: por su codigo
/// unico (es lo que `hojas_que_faltan` copia de la hoja al mensaje) y, si
/// el mensaje no lo tiene, por la pagina, como hace el movil al volver del
/// editor (`it.pagina == paginaDeFondo`).
fn hoja_del_mensaje<'a>(p: &'a mut Proyecto, m: &cuaderno::Mensaje) -> Option<&'a mut Hoja> {
    if let Some(uid) = m.uid.as_deref() {
        return p.hojas.iter_mut().find(|h| h.uid.as_deref() == Some(uid));
    }
    let pagina = m.pagina?;
    p.hojas
        .iter_mut()
        .find(|h| h.pagina == Some(pagina) && h.nota.is_none() && h.croquis.is_none())
}

/// **El dibujo de una hoja-pagina, creandolo la primera vez** (el
/// `Proyectos.conDibujo` del movil al abrir una pagina desde la rejilla).
///
/// Si la hoja ya tiene `dibujo`, se reutiliza (y si su fichero no esta, se
/// crea vacio: el `.excalidraw` lo puede traer luego la sincronizacion). Si
/// no, se le pone `dib-<ahora>` —el mismo formato de id que el movil— se
/// escribe `lienzos/<id>.excalidraw` vacio y se guarda el `proyecto.json`.
/// El fichero va ANTES que el `proyecto.json`: una hoja nunca apunta a un
/// dibujo que no esta.
///
/// `None` si el mensaje no es de una pagina, si el proyecto no tiene
/// `proyecto.json` o si no se pudo escribir: quien abre se queda sin abrir,
/// que es mejor que abrir un lienzo que luego no se guarda en ningun sitio.
pub fn asegurar_dibujo(
    raiz: &Path,
    proyecto: &str,
    m: &cuaderno::Mensaje,
    ahora: i64,
) -> Option<String> {
    if let Some(r) = m.referencia.as_deref().filter(|r| !r.is_empty()) {
        return Some(r.to_string());
    }
    m.pagina?;
    let carpeta = almacen::carpeta(raiz, proyecto);
    let texto = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .inspect_err(|e| tracing::info!(?e, %proyecto, "pagina sin proyecto.json: no hay donde apuntar su dibujo"))
        .ok()?;
    let mut p: Proyecto = serde_json::from_str(&texto)
        .inspect_err(|e| tracing::warn!(?e, %proyecto, "proyecto.json que no se entiende"))
        .ok()?;
    let hoja = hoja_del_mensaje(&mut p, m)?;
    let crear_fichero = |id: &str| -> Option<()> {
        let ruta = almacen::lienzo(raiz, proyecto, id);
        if ruta.is_file() {
            return Some(());
        }
        std::fs::create_dir_all(ruta.parent()?).ok()?;
        let vacio =
            pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::Lienzo::vacio());
        std::fs::write(&ruta, vacio)
            .inspect_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "no se pudo crear el dibujo de la pagina"))
            .ok()
    };
    if let Some(d) = hoja.dibujo.clone() {
        crear_fichero(&d)?;
        return Some(d);
    }
    // Un id que ya este en disco (dos paginas abiertas el mismo
    // milisegundo, o un reloj que fue hacia atras) no se pisa.
    let mut t = ahora;
    let id = loop {
        let id = format!("dib-{t}");
        if !almacen::lienzo(raiz, proyecto, &id).exists() {
            break id;
        }
        t += 1;
    };
    crear_fichero(&id)?;
    hoja.dibujo = Some(id.clone());
    p.tocado = p.tocado.max(ahora);
    guardar_proyecto(raiz, proyecto, &p)
        .inspect_err(|e| tracing::warn!(%e, %proyecto, "no se pudo apuntar el dibujo en la hoja"))
        .ok()?;
    tracing::info!(%proyecto, pagina = ?m.pagina, dibujo = %id, "dibujo nuevo para la pagina");
    Some(id)
}

/// **El dibujo que ya tiene la hoja de ese mensaje**, sin crear nada: el
/// suyo (`referencia`) o el que se le puso a su hoja en el `proyecto.json`
/// (al abrirla aqui o en el movil). Es lo que deja ensenar la pagina con lo
/// dibujado encima en la galeria. Recuerda el `proyecto.json` mientras no
/// cambie: se pregunta por cada pagina al abrir el proyecto.
pub fn dibujo_de_la_hoja(raiz: &Path, proyecto: &str, m: &cuaderno::Mensaje) -> Option<String> {
    if let Some(r) = m.referencia.as_deref().filter(|r| !r.is_empty()) {
        return Some(r.to_string());
    }
    m.pagina?;
    type PorUid = HashMap<String, String>;
    type PorPagina = HashMap<u32, String>;
    type Recuerdo = (PathBuf, Option<std::time::SystemTime>, PorUid, PorPagina);
    thread_local! {
        static CACHE: std::cell::RefCell<Option<Recuerdo>> = const { std::cell::RefCell::new(None) };
    }
    let ruta = almacen::carpeta(raiz, proyecto).join("proyecto.json");
    let fecha = std::fs::metadata(&ruta).and_then(|m| m.modified()).ok();
    CACHE.with_borrow_mut(|c| {
        if !c
            .as_ref()
            .is_some_and(|(r, f, _, _)| *r == ruta && *f == fecha)
        {
            let p: Proyecto = std::fs::read_to_string(&ruta)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
            let mut por_uid = PorUid::new();
            let mut por_pagina = PorPagina::new();
            for h in &p.hojas {
                let Some(d) = h.dibujo.clone() else { continue };
                if let Some(u) = h.uid.clone() {
                    por_uid.insert(u, d.clone());
                }
                if let Some(pg) = h.pagina
                    && h.nota.is_none()
                    && h.croquis.is_none()
                {
                    por_pagina.entry(pg).or_insert(d);
                }
            }
            *c = Some((ruta.clone(), fecha, por_uid, por_pagina));
        }
        let (_, _, por_uid, por_pagina) = c.as_ref()?;
        match m.uid.as_deref() {
            Some(u) => por_uid.get(u).cloned(),
            None => por_pagina.get(&m.pagina?).cloned(),
        }
    })
}

/// **El fondo de una hoja-pagina, sin abrir el PDF**: su documento y, si la
/// galeria ya la tenia pintada, esa vista previa para no abrir en blanco.
/// Lo pinta de verdad el hilo de [`crate::fondo_lienzo`], que tambien
/// descomprime la vista previa: aqui solo se lee la cabecera de su PNG,
/// para saber la proporcion de la hoja.
///
/// Sin documento, la pagina que una version vieja extrajo como PNG
/// (`archivos/pagina-NN.png`), tal cual. `None` si no hay ni eso: se abre
/// el dibujo sin fondo, que es mejor que no abrirlo.
pub fn fondo_de_pagina(
    raiz: &Path,
    proyecto: &str,
    pagina: u32,
) -> Option<crate::fondo_lienzo::Fuente> {
    use crate::fondo_lienzo::{Fuente, PaginaPdf};
    let carpeta = almacen::carpeta(raiz, proyecto);
    let pdf = documento_de(raiz, proyecto)
        .or_else(|| Some(carpeta.join("documento.pdf")).filter(|r| r.is_file()));
    let Some(pdf) = pdf else {
        let vieja = carpeta
            .join("archivos")
            .join(format!("pagina-{:02}.png", pagina + 1));
        return pixpin_codec::cargar(&vieja).ok().map(Fuente::Imagen);
    };
    // Sin lo que el movil cocio dentro: eso se pinta encima, vivo, y en el
    // fondo saldria dos veces (el recuadro doble de una zona vinculada).
    let pdf = crate::pdf_del_proyecto::sin_lo_cocido(raiz, &pdf, pagina);
    // Solo la ruta y la proporcion (la cabecera del PNG): descomprimirlo
    // lo hace el hilo del fondo.
    let previa = pagina_pintada(raiz, &pdf, pagina, ANCHO_VISTA);
    Some(Fuente::Pdf(PaginaPdf {
        pdf,
        pagina,
        proporcion: previa.as_ref().map(|(_, w, h)| h / w.max(1.0)),
        provisional: previa.map(|(png, _, _)| png),
    }))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::imagen::ImagenRgba;

    fn raiz_de_prueba(etiqueta: &str) -> (PathBuf, almacen::Ficha) {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-pdfchat-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = almacen::Ficha::nueva("Obra", 5, "PC01");
        let mut indice = almacen::Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        std::fs::create_dir_all(almacen::carpeta(&raiz, &ficha.id)).unwrap();
        (raiz, ficha)
    }

    fn pdf(raiz: &Path, nombre: &str, paginas: usize) -> PathBuf {
        let imgs: Vec<ImagenRgba> = (0..paginas)
            .map(|i| ImagenRgba {
                ancho: 20,
                alto: 30,
                pixeles: [(i * 40) as u8, 90, 200, 255].repeat(600),
            })
            .collect();
        let ruta = raiz.join(nombre);
        std::fs::write(&ruta, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
        ruta
    }

    fn proyecto(raiz: &Path, ficha: &almacen::Ficha) -> Proyecto {
        leer_proyecto(raiz, ficha)
    }

    #[test]
    fn unir_un_pdf_lo_hace_documento_y_cada_pagina_una_hoja_que_apunta_a_el() {
        let (raiz, ficha) = raiz_de_prueba("unir");
        let doc = pdf(&raiz, "plano.pdf", 3);
        let u = unir(&raiz, &ficha, &doc, "m1", "plano", 1000, &|_, _| {}).unwrap();
        assert_eq!((u.hojas, u.como), (3, Como::Documento));
        let p = proyecto(&raiz, &ficha);
        assert_eq!(p.pdf_origen.as_deref(), Some("archivos/doc-1000.pdf"));
        assert_eq!(p.pdf_limpio.as_deref(), Some("archivos/limpio-1000.pdf"));
        let paginas: Vec<_> = p.hojas.iter().map(|h| h.pagina).collect();
        assert_eq!(paginas, vec![Some(0), Some(1), Some(2)]);
        for h in &p.hojas {
            // Ni dibujo ni foto: la hoja ES la pagina del PDF.
            assert!(h.dibujo.is_none());
            assert_eq!(
                h.resto.get("deMensaje").and_then(|v| v.as_str()),
                Some("m1")
            );
            assert!(h.uid.is_some());
        }
        // Y ninguna imagen extraida: ni en imagenes/ ni en archivos/ salvo el PDF.
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        assert!(!carpeta.join("imagenes").exists());
        let pngs = std::fs::read_dir(carpeta.join("archivos"))
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .count();
        assert_eq!(pngs, 0, "unir no extrae paginas como imagenes");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn con_documento_las_paginas_nuevas_se_pegan_detras_como_paginas_de_verdad() {
        let (raiz, ficha) = raiz_de_prueba("pegar");
        let a = pdf(&raiz, "a.pdf", 2);
        let b = pdf(&raiz, "b.pdf", 3);
        unir(&raiz, &ficha, &a, "m1", "a", 1000, &|_, _| {}).unwrap();
        let u = unir(&raiz, &ficha, &b, "m2", "b", 2000, &|_, _| {}).unwrap();
        assert_eq!((u.hojas, u.como), (3, Como::Pegadas));
        let p = proyecto(&raiz, &ficha);
        let paginas: Vec<_> = p.hojas.iter().filter_map(|h| h.pagina).collect();
        assert_eq!(paginas, vec![0, 1, 2, 3, 4]);
        let doc = pixpin_proyecto::vista::documento_del_proyecto(&raiz, &ficha.id).unwrap();
        assert_eq!(
            pixpin_pdf::union::contar_paginas(&std::fs::read(&doc).unwrap()),
            Some(5)
        );
        // La copia limpia va a la par.
        let limpio = almacen::carpeta(&raiz, &ficha.id).join("archivos/limpio-1000.pdf");
        assert_eq!(
            pixpin_pdf::union::contar_paginas(&std::fs::read(limpio).unwrap()),
            Some(5)
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_pdf_cifrado_cae_a_paginas_pintadas_pegadas_al_documento() {
        let (raiz, ficha) = raiz_de_prueba("cifrado");
        let a = pdf(&raiz, "a.pdf", 1);
        unir(&raiz, &ficha, &a, "m1", "a", 1000, &|_, _| {}).unwrap();
        // Cifrado sin clave de usuario: Windows lo pinta, pero sus objetos
        // no se dejan copiar.
        let b = pdf(&raiz, "b.pdf", 2);
        let texto = std::fs::read(&b).unwrap();
        let texto = String::from_utf8_lossy(&texto)
            .replace("/Root 1 0 R>>", "/Root 1 0 R /Encrypt 99 0 R>>")
            .into_bytes();
        std::fs::write(&b, texto).unwrap();
        assert!(pixpin_pdf::union::esta_cifrado(&std::fs::read(&b).unwrap()));
        let pasos = std::cell::Cell::new(0);
        let u = unir(&raiz, &ficha, &b, "m2", "b", 2000, &|_, _| {
            pasos.set(pasos.get() + 1)
        });
        let u = u.unwrap();
        assert_eq!(u.como, Como::Pintadas);
        assert_eq!(u.hojas, 2);
        assert!(pasos.get() > 0, "pintar dice por donde va");
        let p = proyecto(&raiz, &ficha);
        assert_eq!(
            p.hojas.iter().filter_map(|h| h.pagina).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn como_imagenes_respeta_el_tope_y_deja_fotos_clavadas() {
        let (raiz, ficha) = raiz_de_prueba("fotos");
        let doc = pdf(&raiz, "largo.pdf", TOPE_DE_PAGINAS_COMO_FOTO as usize + 3);
        let u = como_imagenes(&raiz, &ficha, &doc, "m1", "largo", 1000, &|_, _| {}).unwrap();
        assert_eq!(u.como, Como::Fotos);
        assert_eq!(u.hojas, TOPE_DE_PAGINAS_COMO_FOTO as usize);
        assert_eq!(u.paginas, TOPE_DE_PAGINAS_COMO_FOTO + 3);
        let p = proyecto(&raiz, &ficha);
        assert_eq!(p.hojas.len(), TOPE_DE_PAGINAS_COMO_FOTO as usize);
        // No toca el documento del proyecto: son fotos, no paginas.
        assert!(p.pdf_origen.is_none());
        let h = &p.hojas[0];
        assert_eq!(h.nombre, "largo 1");
        let texto = std::fs::read_to_string(almacen::lienzo(
            &raiz,
            &ficha.id,
            h.dibujo.as_ref().unwrap(),
        ))
        .unwrap();
        let lienzo = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
        let ficheros = pixpin_motor2d::excalidraw::ficheros(&lienzo);
        assert_eq!(ficheros.len(), 1);
        assert!(ficheros[0].1.starts_with("imagenes/pag1000x0"));
        assert!(
            almacen::carpeta(&raiz, &ficha.id)
                .join(&ficheros[0].1)
                .is_file()
        );
        assert!(texto.contains("\"locked\":true"), "la foto va clavada");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn soltar_un_pdf_de_cien_paginas_no_bloquea_la_ventana() {
        // Lo que hace el hilo de la ventana al soltar un PDF: copiarlo al
        // proyecto como adjunto y pedir la vista de su primera pagina. Nada
        // de pintar paginas: eso va en su hilo.
        let (raiz, ficha) = raiz_de_prueba("soltar");
        let doc = pdf(&raiz, "cien.pdf", 100);
        let bytes = std::fs::read(&doc).unwrap();
        let t = std::time::Instant::now();
        let relativa = almacen::guardar_adjunto(&raiz, &ficha.id, "cien.pdf", &bytes).unwrap();
        let guardado = almacen::carpeta(&raiz, &ficha.id).join(&relativa);
        let vista = pagina_pintada(&raiz, &guardado, 0, ANCHO_VISTA);
        let tardo = t.elapsed();
        assert!(
            vista.is_none(),
            "la primera vez aun no esta pintada: se pide y se sigue"
        );
        assert!(
            tardo.as_millis() < 50,
            "el hilo de la ventana se paro {tardo:?}"
        );
        // Y no se extrajo ninguna pagina en el proyecto.
        let ficheros = std::fs::read_dir(almacen::carpeta(&raiz, &ficha.id).join("archivos"))
            .unwrap()
            .count();
        assert_eq!(ficheros, 1, "solo el PDF");
        // Al rato, la vista llega sola desde su hilo.
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut llego = None;
        while llego.is_none() && std::time::Instant::now() < hasta {
            std::thread::sleep(std::time::Duration::from_millis(20));
            llego = pagina_pintada(&raiz, &guardado, 0, ANCHO_VISTA);
        }
        let (png, w, h) = llego.expect("la vista previa tiene que llegar");
        assert_eq!(w, ANCHO_PAPEL_PDF, "el ancho del papel del movil");
        assert!((h / w - 1.5).abs() < 0.02, "la proporcion de la pagina");
        assert_eq!(pixpin_codec::imagen::medidas(&png).unwrap().0, ANCHO_VISTA);
        // Fuera de la carpeta del proyecto: no viaja al empaquetar.
        assert!(!png.starts_with(almacen::carpeta(&raiz, &ficha.id)));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn pedir_mas_ancho_da_otra_pagina_pintada_y_no_estira_la_pequena() {
        // El zoom pide mas resolucion: la cache es por ancho, asi que una
        // pagina pedida grande se pinta aparte de la vista previa.
        let raiz = std::env::temp_dir().join(format!("pixpin-pdfchat-zoom-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let doc = pdf(&raiz, "z.pdf", 1);
        let pequena = en_cache(&raiz, &doc, 0, ANCHO_VISTA).unwrap();
        let grande = en_cache(&raiz, &doc, 0, ANCHO_VISTA * 4).unwrap();
        assert_ne!(pequena, grande);
        // Caso negativo: otra pagina u otro PDF no comparten sitio.
        assert_ne!(pequena, en_cache(&raiz, &doc, 1, ANCHO_VISTA).unwrap());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_hojas_de_un_mensaje_del_chat_van_solo_a_la_galeria() {
        let (raiz, ficha) = raiz_de_prueba("galeria");
        let doc = pdf(&raiz, "g.pdf", 2);
        unir(&raiz, &ficha, &doc, "m1", "g", 1000, &|_, _| {}).unwrap();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let sello = cuaderno::Sello {
            cuando: 1,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        };
        let mut pdf_msg = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "g.pdf",
            "archivos/g.pdf",
            1,
            &sello,
        );
        pdf_msg.id = "m1".into();
        let mut mensajes = vec![pdf_msg];
        mensajes.extend(almacen::hojas_para_ensenar(&raiz, &ficha.id, "PC01"));
        assert_eq!(mensajes.len(), 3);
        marcar_solo_galeria(&carpeta, &mut mensajes, 1);
        assert!(!solo_galeria(&mensajes[0]), "el PDF sigue en el chat");
        assert!(mensajes[1..].iter().all(solo_galeria));
        // La cabecera cuenta solo lo que se ve: el PDF, no sus paginas.
        assert_eq!(cuenta_de_la_cabecera(&mensajes).0, 1);
        assert_eq!(cuenta_de_la_cabecera(&[]), (0, 0));
        // Y las paginas no salen ni aunque el mensaje del PDF no este en
        // este chat: una pagina del PDF nunca es un mensaje (movil).
        let mut solas: Vec<_> = almacen::hojas_para_ensenar(&raiz, &ficha.id, "PC01");
        marcar_solo_galeria(&carpeta, &mut solas, 0);
        assert!(solas.iter().all(solo_galeria));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_fotos_de_un_pdf_cuyo_mensaje_no_esta_en_el_chat_si_se_ensenan() {
        // Caso negativo: pasado a imagenes, cada pagina es un lienzo con su
        // foto, no una pagina del PDF. Si su mensaje no esta en este chat
        // (vinieron de otro sitio), son lienzos sueltos y salen, como en el
        // `RegistroDelChat` del movil.
        let (raiz, ficha) = raiz_de_prueba("fotos-solas");
        let doc = pdf(&raiz, "f.pdf", 2);
        como_imagenes(&raiz, &ficha, &doc, "m-de-otro-chat", "f", 1000, &|_, _| {}).unwrap();
        let mut solas: Vec<_> = almacen::hojas_para_ensenar(&raiz, &ficha.id, "PC01");
        assert_eq!(solas.len(), 2);
        marcar_solo_galeria(&almacen::carpeta(&raiz, &ficha.id), &mut solas, 0);
        assert!(!solas.iter().any(solo_galeria));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn sello(ficha: &almacen::Ficha, n: i64) -> cuaderno::Sello {
        cuaderno::Sello {
            cuando: 100 + n,
            numero: n,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        }
    }

    /// Lo que el chat ensena de verdad: lo del cuaderno y lo que solo se
    /// ensena, sin lo que va solo a la galeria.
    fn lo_que_se_ve(raiz: &Path, ficha: &almacen::Ficha) -> Vec<cuaderno::Mensaje> {
        let mut mensajes = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id))
            .map(|c| c.mensajes)
            .unwrap_or_default();
        para_el_chat(raiz, &ficha.id, "PC01", &mut mensajes);
        mensajes.into_iter().filter(|m| !solo_galeria(m)).collect()
    }

    #[test]
    fn meter_un_pdf_de_cinco_paginas_deja_un_mensaje_en_el_chat_y_cinco_hojas_en_el_proyecto() {
        let (raiz, ficha) = raiz_de_prueba("cinco");
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let doc = pdf(&raiz, "plano.pdf", 5);
        let mut m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "plano.pdf",
            "archivos/plano.pdf",
            1,
            &sello(&ficha, 1),
        );
        m.id = "m-pdf".into();
        cuaderno::anadir(&carpeta, &m).unwrap();
        unir(&raiz, &ficha, &doc, "m-pdf", "plano", 1000, &|_, _| {}).unwrap();
        // Una pagina anotada (con su dibujo) tampoco sale.
        let mut p = proyecto(&raiz, &ficha);
        p.hojas[3].dibujo = Some("dib-anotada".into());
        guardar_proyecto(&raiz, &ficha.id, &p).unwrap();

        let visto = lo_que_se_ve(&raiz, &ficha);
        assert_eq!(
            visto.len(),
            1,
            "solo el PDF: {:?}",
            visto.iter().map(|m| &m.nombre).collect::<Vec<_>>()
        );
        assert_eq!(visto[0].id, "m-pdf");
        let p = proyecto(&raiz, &ficha);
        assert_eq!(p.hojas.iter().filter(|h| h.pagina.is_some()).count(), 5);
        // Y abrir el proyecto no las escribe en el cuaderno (`completar_hojas`).
        almacen::completar_hojas(&raiz, &ficha.id, "PC01").unwrap();
        let escritos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap().mensajes;
        assert_eq!(escritos.len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_cuaderno_viejo_con_mensajes_de_pagina_no_los_ensena_pero_no_los_borra() {
        // Un proyecto del movil (paginas sin `deMensaje`) cuyo cuaderno ya
        // tiene un mensaje por pagina, escrito por una version anterior.
        let (raiz, ficha) = raiz_de_prueba("viejo");
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let pagina = |n: u32| Hoja {
            id: format!("h-{n}"),
            nombre: format!("Pagina {}", n + 1),
            pagina: Some(n),
            uid: Some(format!("PAG{n}")),
            ..Default::default()
        };
        let mut hojas: Vec<Hoja> = (0..3).map(pagina).collect();
        hojas.push(Hoja {
            id: "h-lienzo".into(),
            nombre: "Croquis".into(),
            dibujo: Some("dib-croquis".into()),
            uid: Some("LIENZO".into()),
            ..Default::default()
        });
        // Una pagina que el usuario SI mando al chat y luego unio: su hoja
        // sale de ese mensaje y lleva su codigo.
        let mut unida = pagina(4);
        unida.uid = Some("MANDADA".into());
        unida.resto.insert(
            "deMensaje".into(),
            serde_json::Value::String("m-mandada".into()),
        );
        hojas.push(unida);
        let p = Proyecto {
            id: ficha.id.clone(),
            hojas,
            ..Default::default()
        };
        guardar_proyecto(&raiz, &ficha.id, &p).unwrap();

        let mut n = 0;
        let mut escribir = |clase, nombre: &str, uid: &str, id: &str, pag: Option<u32>| {
            n += 1;
            let mut m = cuaderno::Mensaje::adjunto(clase, nombre, "", 0, &sello(&ficha, n));
            m.uid = Some(uid.into());
            m.id = id.into();
            m.pagina = pag;
            cuaderno::anadir(&carpeta, &m).unwrap();
        };
        for i in 0..3 {
            escribir(
                cuaderno::Clase::Pagina,
                "Pagina",
                &format!("PAG{i}"),
                &format!("m-pag-{i}"),
                Some(i),
            );
        }
        escribir(
            cuaderno::Clase::Dibujo,
            "Croquis",
            "LIENZO",
            "m-lienzo",
            None,
        );
        escribir(cuaderno::Clase::Imagen, "foto.png", "FOTO", "m-foto", None);
        // «Una pagina de un proyecto» (A7): un mensaje nuevo, con codigo propio.
        escribir(
            cuaderno::Clase::Pagina,
            "Plano · Pagina 2",
            "A7A7A7",
            "m-a7",
            Some(1),
        );
        escribir(
            cuaderno::Clase::Pagina,
            "Pagina 5",
            "MANDADA",
            "m-mandada",
            Some(4),
        );

        let visto: Vec<String> = lo_que_se_ve(&raiz, &ficha)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(visto, ["m-lienzo", "m-foto", "m-a7", "m-mandada"]);
        // Esconder no es borrar: el cuaderno viaja al movil.
        assert_eq!(
            cuaderno::Cuaderno::leer_de(&carpeta)
                .unwrap()
                .mensajes
                .len(),
            7
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_punto_rojo_sale_solo_si_se_unio_y_ya_no_esta_su_hoja() {
        let (raiz, ficha) = raiz_de_prueba("punto");
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let sello = cuaderno::Sello {
            cuando: 1,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        };
        let mut m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "g.pdf",
            "archivos/g.pdf",
            1,
            &sello,
        );
        m.id = "m1".into();
        assert!(!sin_su_hoja(&carpeta, &m), "sin unir no hay punto");
        m.resto
            .insert("unido".into(), serde_json::Value::Bool(true));
        assert!(sin_su_hoja(&carpeta, &m), "unido y sin hoja: punto");
        let doc = pdf(&raiz, "g.pdf", 1);
        // La fecha del fichero cambia de segundo en algunos discos: se
        // espera un poco para que la cache note que cambio.
        std::thread::sleep(std::time::Duration::from_millis(20));
        unir(&raiz, &ficha, &doc, "m1", "g", 1000, &|_, _| {}).unwrap();
        assert!(!sin_su_hoja(&carpeta, &m), "con su hoja no hay punto");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_lienzo_del_movil_con_su_hoja_no_lleva_punto_rojo_aunque_los_ids_no_coincidan() {
        let (raiz, ficha) = raiz_de_prueba("punto-lienzo");
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let p = Proyecto {
            id: ficha.id.clone(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "h-1".into(),
                dibujo: Some("dib-1".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_vec(&p).unwrap(),
        )
        .unwrap();
        let mut m = cuaderno::Mensaje {
            id: "registro-x-dib-1".into(),
            clase: Some(cuaderno::Clase::Dibujo),
            referencia: Some("dib-1".into()),
            ..Default::default()
        };
        m.resto
            .insert("unido".into(), serde_json::Value::Bool(true));
        assert!(!sin_su_hoja(&carpeta, &m), "su hoja es la del mismo dibujo");
        // Caso negativo: un lienzo cuyo dibujo no tiene hoja si lo lleva.
        m.referencia = Some("dib-9".into());
        assert!(sin_su_hoja(&carpeta, &m));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
