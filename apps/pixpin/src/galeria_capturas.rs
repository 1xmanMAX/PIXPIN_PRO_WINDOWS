//! **La galeria de capturas**: todas las capturas guardadas, la mas nueva
//! primero, con su miniatura y lo que se hace con una (2-oct).
//!
//! El usuario la pidio al cambiar la pila por el interruptor: «aun asi que la
//! app muestre una galeria de todas las capturas». En el movil no hay una
//! pantalla igual: alli las capturas van a la galeria del sistema
//! (`capture/Export.kt`, `Pictures/PixPin`). En Windows no hay galeria del
//! sistema que valga, asi que esta ventana hace ese papel sobre la carpeta
//! `capturas/` de los datos, que es donde caen las capturas copiadas (la
//! pila escribe cada una), las guardadas, los GIF y los MP4.
//!
//! Ligera a proposito, que tiene que ir en un equipo de 4 GB con grafica
//! integrada:
//!
//! - **Solo se pinta lo que se ve.** La rejilla es virtual: de mil capturas
//!   se piden las miniaturas de las filas a la vista y poco mas.
//! - **Las miniaturas se sacan en otro hilo** y se guardan reducidas en
//!   `cache/galeria/`: abrir la galeria la segunda vez no vuelve a
//!   descomprimir capturas de pantalla enteras.
//! - **En memoria, un tope.** Las que quedan lejos de la vista se sueltan.
//! - La carpeta se relee solo si cambio (su fecha de modificacion).
//!
//! «A la papelera» mueve el fichero a `papelera/capturas/` de los datos, de
//! donde se puede sacar a mano: nada se borra de verdad desde aqui, y por
//! eso borrar no pregunta: avisa con «Deshacer» (Ctrl+Z).
//!
//! **La v2** (maqueta `Galeria2.dc.html`, 4-oct): filtros con su numero a
//! la izquierda, buscador (que tambien encuentra el texto de dentro de las
//! capturas, leido con el OCR de Windows en segundo plano), las capturas
//! agrupadas por dia, la caducidad por colores (gris, naranja, rojo y verde
//! de conservada), un modo de elegir varias con su barra de acciones, y un
//! panel de detalle con «Conservar», «Dar 7 dias mas» y el texto reconocido.
//! Lo que se decide sin pintar vive en [`logica`].

#![forbid(unsafe_code)]

mod logica;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use logica::{Filtro, Plazo, Tono};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;
use pixpin_render::icono::Icono;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, EstiloTexto, Pintor, RectF, Superficie, Tramo};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, centrado, dentro};

/// Lado mayor de la miniatura guardada: la celda mide hasta ~230 logicos
/// de ancho; con 256 no se ve pastosa y pesa 256 KB como mucho.
const LADO: u32 = 256;
/// Cuantas miniaturas se tienen en memoria a la vez, como mucho. Una
/// pantalla llena son ~25; el resto es colchon para volver atras sin
/// recargar. 160 de 256 px son ~40 MB en el peor caso.
const TOPE_EN_MEMORIA: usize = 160;
/// Las extensiones que se listan. El MP4 tambien: no tiene miniatura, pero
/// se abre y se lleva a la papelera como las demas.
const EXTENSIONES: [&str; 7] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4"];

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana: una sola
/// galeria a la vez, y pedirla otra vez la trae delante.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Abre la galeria, o la trae delante si ya estaba. Vuelve enseguida: la
/// ventana vive en su propio hilo.
pub fn abrir(idioma: Idioma, ubicacion: Ubicacion) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("galeria-capturas".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = Recursos::nuevos().and_then(|r| bucle(&r, &textos, &ubicacion));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la galeria de capturas");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la galeria");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// --------------------------------------------------------------- la lista

/// Una captura de la carpeta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub ruta: PathBuf,
    pub cuando: SystemTime,
    pub bytes: u64,
}

/// La carpeta de las capturas: la misma en que las escribe `main.rs`.
pub fn carpeta(ubicacion: &Ubicacion) -> PathBuf {
    carpeta_en(ubicacion.raiz())
}

/// La de [`carpeta`], desde la raiz de los datos.
pub fn carpeta_en(raiz: &Path) -> PathBuf {
    raiz.join("capturas")
}

/// Si una ruta es de las que lista la galeria (por su extension).
pub fn es_captura(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONES.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Si se le puede sacar miniatura (todo menos el video).
fn tiene_miniatura(ruta: &Path) -> bool {
    !ruta
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

/// Las capturas de `carpeta`, la mas nueva primero. Una carpeta que no
/// existe es una lista vacia, no un error: aun no se capturo nada.
pub fn listar(carpeta: &Path) -> Vec<Entrada> {
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    let mut v: Vec<Entrada> = lista
        .flatten()
        .filter_map(|e| {
            let ruta = e.path();
            let meta = e.metadata().ok()?;
            // Una ruta reservada y aun vacia es una captura escribiendose:
            // saldra en la siguiente relectura.
            (meta.is_file() && meta.len() > 0 && es_captura(&ruta)).then(|| Entrada {
                ruta,
                cuando: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                bytes: meta.len(),
            })
        })
        .collect();
    ordenar(&mut v);
    v
}

/// La mas nueva primero; a igual hora, por nombre al reves (`captura-0193`
/// antes que `captura-0192`). Por fecha y no por nombre: el numero se
/// reutiliza cuando se borra una captura vieja.
fn ordenar(v: &mut [Entrada]) {
    v.sort_by(|a, b| b.cuando.cmp(&a.cuando).then_with(|| b.ruta.cmp(&a.ruta)));
}

/// Donde se guarda la miniatura de una captura: cambia si cambia la captura
/// (fecha o tamano), asi que una vieja nunca se ensena por una nueva.
fn ruta_en_cache(cache: &Path, e: &Entrada) -> PathBuf {
    let segundos = e
        .cuando
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let nombre = e
        .ruta
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    cache.join(format!("{nombre}-{segundos}-{}.png", e.bytes))
}

/// Lleva una captura a `papelera/capturas/`. Si ya hay una con ese nombre,
/// la nueva lleva la hora delante: nada se pisa.
pub fn a_la_papelera(raiz: &Path, ruta: &Path) -> std::io::Result<PathBuf> {
    let dir = raiz.join("papelera").join("capturas");
    std::fs::create_dir_all(&dir)?;
    let nombre = ruta
        .file_name()
        .ok_or_else(|| std::io::Error::other("sin nombre"))?;
    let mut destino = dir.join(nombre);
    if destino.exists() {
        let ms = pixpin_shell::entorno::ahora_utc_ms();
        destino = dir.join(format!("{ms}-{}", nombre.to_string_lossy()));
    }
    std::fs::rename(ruta, &destino)?;
    Ok(destino)
}

/// **Borra una captura** como el boton «Borrar» de la galeria: a la
/// papelera de PixPin y, si estaba conservada, se olvida (su nombre queda
/// libre: una captura nueva que lo herede no nace conservada). Devuelve el
/// registro si cambio.
pub(crate) fn borrar(
    raiz: &Path,
    ruta: &Path,
) -> std::io::Result<Option<crate::caducidad_capturas::Registro>> {
    // El mismo camino que el borrado de la galeria (que se puede deshacer):
    // tambien olvida su prorroga si la tenia.
    let (_, reg, error) = llevar_a_la_papelera(raiz, &[ruta.to_path_buf()]);
    match error {
        Some(m) => Err(std::io::Error::other(m)),
        None => Ok(reg),
    }
}

// ----------------------------------------------------------- miniaturas

/// Lo que devuelve el hilo de las miniaturas.
/// Con la miniatura van las medidas de verdad de la captura (leidas de su
/// cabecera, sin descomprimirla), que salen en el panel de detalle.
type Hecha = (PathBuf, Option<ImagenRgba>, Option<(u32, u32)>);

enum Mini {
    Pedida,
    Lista {
        bitmap: Option<ID2D1Bitmap1>,
        imagen: ImagenRgba,
        visto: u64,
    },
    Imposible,
}

/// Lanza el hilo que lee y reduce. Atiende primero lo ULTIMO que se pidio:
/// es lo que esta a la vista ahora, no lo que se vio al pasar desplazando.
fn lanzar_lector(cache: PathBuf, aviso: isize) -> (Sender<(Entrada, PathBuf)>, Receiver<Hecha>) {
    let (pedir, pedidos) = channel::<(Entrada, PathBuf)>();
    let (dar, hechas) = channel::<Hecha>();
    let _ = std::thread::Builder::new()
        .name("galeria-miniaturas".into())
        .spawn(move || {
            let _ = std::fs::create_dir_all(&cache);
            let mut cola: Vec<(Entrada, PathBuf)> = Vec::new();
            loop {
                if cola.is_empty() {
                    match pedidos.recv() {
                        Ok(p) => cola.push(p),
                        Err(_) => return,
                    }
                }
                while let Ok(p) = pedidos.try_recv() {
                    cola.push(p);
                }
                let Some((entrada, en_cache)) = cola.pop() else {
                    continue;
                };
                let mini = leer_reducida(&entrada.ruta, &en_cache);
                let medidas = pixpin_codec::imagen::medidas(&entrada.ruta).ok();
                if dar.send((entrada.ruta, mini, medidas)).is_err() {
                    return;
                }
                if aviso != 0 {
                    pixpin_shell::overlay::despertar(aviso);
                }
            }
        });
    (pedir, hechas)
}

/// La miniatura de una captura: de la cache si esta; si no, se lee entera,
/// se reduce y se guarda en la cache para la proxima vez.
fn leer_reducida(ruta: &Path, en_cache: &Path) -> Option<ImagenRgba> {
    if let Ok(m) = pixpin_codec::imagen::cargar(en_cache) {
        return Some(m);
    }
    let entera = pixpin_codec::imagen::cargar(ruta).ok()?;
    let (w, h) = (entera.ancho, entera.alto);
    if w == 0 || h == 0 {
        return None;
    }
    let mayor = w.max(h);
    let reducida = if mayor > LADO {
        let f = LADO as f64 / mayor as f64;
        let nw = ((w as f64 * f).round() as u32).max(1);
        let nh = ((h as f64 * f).round() as u32).max(1);
        pixpin_codec::redimensionar(entera, nw, nh).ok()?
    } else {
        entera
    };
    if let Err(e) = pixpin_codec::guardar(&reducida, en_cache, pixpin_codec::FormatoImagen::Png) {
        tracing::debug!(?e, "la miniatura no se pudo guardar en la cache");
    }
    Some(reducida)
}

// ------------------------------------------------------ el texto (OCR)

/// Lo que devuelve el hilo que lee el texto de las capturas.
enum Leido {
    /// Este Windows no reconoce texto: no se pedira nada mas.
    SinOcr,
    Texto(PathBuf, String),
}

/// Donde se guarda el texto ya reconocido de una captura: junto a su
/// miniatura, con el mismo nombre que cambia si cambia la captura.
fn ruta_del_texto(cache: &Path, e: &Entrada) -> PathBuf {
    ruta_en_cache(cache, e).with_extension("txt")
}

/// **Lee el texto de las capturas en segundo plano**, con el OCR que trae
/// Windows (`pixpin_ocr`), para buscar por lo que pone dentro y para el
/// panel de detalle. Una captura de pantalla tarda de 170 a 670 ms (medido
/// en `main.rs`): por eso va en su hilo, de una en una, con un respiro entre
/// una y otra para no quitarle el procesador al equipo, y lo leido se
/// guarda en la cache: la segunda vez que se abre la galeria no se relee.
///
/// Como las miniaturas, atiende primero lo ULTIMO pedido: la ventana pide
/// todas al abrir (de la mas vieja a la mas nueva) y luego, cada vez que se
/// elige una, esa, que asi sale enseguida en el detalle.
fn lanzar_lector_de_texto(cache: PathBuf, aviso: isize) -> (Sender<Entrada>, Receiver<Leido>) {
    let (pedir, pedidos) = channel::<Entrada>();
    let (dar, leidos) = channel::<Leido>();
    let _ = std::thread::Builder::new()
        .name("galeria-texto".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let despertar = || {
                if aviso != 0 {
                    pixpin_shell::overlay::despertar(aviso);
                }
            };
            if !pixpin_ocr::disponible() {
                let _ = dar.send(Leido::SinOcr);
                despertar();
                return;
            }
            let _ = std::fs::create_dir_all(&cache);
            let mut hechos: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
            let mut cola: Vec<Entrada> = Vec::new();
            loop {
                if cola.is_empty() {
                    match pedidos.recv() {
                        Ok(p) => cola.push(p),
                        Err(_) => return,
                    }
                }
                while let Ok(p) = pedidos.try_recv() {
                    cola.push(p);
                }
                let Some(entrada) = cola.pop() else {
                    continue;
                };
                if !hechos.insert(entrada.ruta.clone()) {
                    continue;
                }
                let guardado = ruta_del_texto(&cache, &entrada);
                let texto = match std::fs::read_to_string(&guardado) {
                    Ok(t) => t,
                    Err(_) => {
                        let Ok(img) = pixpin_codec::imagen::cargar(&entrada.ruta) else {
                            // No se pudo abrir (se borro, esta a medias): ni
                            // texto ni cache, por si la proxima vez si.
                            continue;
                        };
                        let t = crate::texto_de_imagen(&img).unwrap_or_default();
                        if let Err(e) = std::fs::write(&guardado, &t) {
                            tracing::debug!(?e, "el texto no se pudo guardar en la cache");
                        }
                        std::thread::sleep(Duration::from_millis(40));
                        t
                    }
                };
                if dar.send(Leido::Texto(entrada.ruta, texto)).is_err() {
                    return;
                }
                despertar();
            }
        });
    (pedir, leidos)
}

// --------------------------------------------------------------- la ventana

// Los colores de la maqueta v2 (`Galeria2.dc.html`). La galeria es oscura
// como las demas ventanitas (`ventanita`), que no tienen tema claro.
const FONDO_V: Color = hex(0x1C1C1E);
const LATERAL: Color = hex(0x232326);
const BARRA_ELEGIDAS: Color = hex(0x2C2C2E);
const AZUL_V: Color = hex(0x0A84FF);
const AZUL_PRI: Color = hex(0x0060DF);
const VERDE_V: Color = hex(0x248A3D);
const NARANJA: Color = hex(0xFF9F0A);
const ROJO_V: Color = hex(0xC9342B);
const ROJO_TEXTO: Color = hex(0xFF6961);
const ENLACE: Color = hex(0x64D2FF);
const GRIS: Color = hex(0x98989D);
const CLARO: Color = hex(0xF5F5F7);
const TINTA_OSCURA: Color = hex(0x1C1C1E);
const TECLA_DOBLE: Duration = Duration::from_millis(500);

fn blanco(a: f32) -> Color {
    Color { a, ..Color::BLANCO }
}

fn alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    /// Una zona que se traga el clic (el fondo de un panel).
    Nada,
    Cerrar,
    Carpeta,
    Papelera,
    Mover,
    Buscar,
    LimpiarBusqueda,
    Filtro(Filtro),
    AlternarEligiendo,
    SalirEligiendo,
    ElegirDia(i64),
    ElegirTodas,
    /// Las de abajo llevan la posicion en `lista`.
    Celda(usize),
    Marca(usize),
    Pinear(usize),
    Copiar(usize),
    Conservar(usize),
    Prorrogar(usize),
    BorrarUna(usize),
    VerGrande(usize),
    CopiarTexto(usize),
    /// La barra de las elegidas.
    BarraConservar,
    BarraPinear,
    BarraCopiar,
    BarraBorrar,
    Deshacer,
}

struct Aviso {
    texto: String,
    desde: Instant,
    /// Con boton de «Deshacer» (y dura mas).
    deshacer: bool,
}

/// Una captura llevada a la papelera de PixPin, con lo que hace falta para
/// devolverla tal cual estaba.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Borrada {
    pub original: PathBuf,
    pub destino: PathBuf,
    pub conservada: bool,
    pub prorroga: Option<i64>,
}

struct Estado {
    lista: Vec<Entrada>,
    /// Cuando se va cada una y cuales se conservan (`caducidad_capturas`).
    registro: crate::caducidad_capturas::Registro,
    minis: HashMap<PathBuf, Mini>,
    /// Ancho y alto de verdad de cada captura (no los de la miniatura).
    medidas: HashMap<PathBuf, (u32, u32)>,
    /// El texto reconocido de cada una, tal cual y plegado para buscar.
    leidos: HashMap<PathBuf, (String, String)>,
    /// Si hay OCR: `None` mientras no se sabe.
    ocr: Option<bool>,
    filtro: Filtro,
    consulta: String,
    /// El cursor esta en el buscador: las letras van ahi.
    escribiendo: bool,
    /// Las posiciones de `lista` que se ven, en orden.
    vista: Vec<usize>,
    disp: logica::Disposicion,
    foco: Option<PathBuf>,
    eligiendo: bool,
    elegidas: BTreeSet<PathBuf>,
    scroll: f32,
    botones: Botones<Accion>,
    aviso: Option<Aviso>,
    /// Lo ultimo borrado, para «Deshacer».
    deshacer: Vec<Borrada>,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    ultimo_clic: Option<(Accion, Instant)>,
    en_papelera: usize,
    fotograma: u64,
    ahora: i64,
}

impl Estado {
    fn nuevo(lista: Vec<Entrada>, registro: crate::caducidad_capturas::Registro) -> Self {
        Estado {
            lista,
            registro,
            minis: HashMap::new(),
            medidas: HashMap::new(),
            leidos: HashMap::new(),
            ocr: None,
            filtro: Filtro::Todas,
            consulta: String::new(),
            escribiendo: false,
            vista: Vec::new(),
            disp: logica::disponer(&[], 0.0, 1.0),
            foco: None,
            eligiendo: false,
            elegidas: BTreeSet::new(),
            scroll: 0.0,
            botones: Botones::default(),
            aviso: None,
            deshacer: Vec::new(),
            moviendo: None,
            ultimo_clic: None,
            en_papelera: 0,
            fotograma: 0,
            ahora: pixpin_shell::entorno::ahora_utc_ms(),
        }
    }

    fn se_va(&self, i: usize) -> Option<i64> {
        crate::caducidad_capturas::se_va_el(&self.registro, &self.lista[i])
    }

    fn ficha(&self, i: usize) -> logica::Ficha<'_> {
        let e = &self.lista[i];
        logica::Ficha {
            extension: e.ruta.extension().and_then(|x| x.to_str()).unwrap_or(""),
            cuando: crate::caducidad_capturas::ms_de(e.cuando),
            se_va: self.se_va(i),
            texto: self.leidos.get(&e.ruta).map(|(t, _)| t.as_str()),
        }
    }

    fn indice(&self, ruta: &Path) -> Option<usize> {
        self.lista.iter().position(|x| x.ruta == ruta)
    }

    fn foco_i(&self) -> Option<usize> {
        self.foco.as_deref().and_then(|r| self.indice(r))
    }

    /// La posicion del foco en la vista.
    fn foco_en_vista(&self) -> Option<usize> {
        let i = self.foco_i()?;
        self.vista.iter().position(|&v| v == i)
    }

    /// Con que se hace lo de la barra o de una tecla: las elegidas si se
    /// esta eligiendo y hay alguna; si no, la del foco.
    fn objetivos(&self) -> Vec<usize> {
        if self.eligiendo && !self.elegidas.is_empty() {
            return (0..self.lista.len())
                .filter(|&i| self.elegidas.contains(&self.lista[i].ruta))
                .collect();
        }
        self.foco_i().into_iter().collect()
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some(Aviso {
            texto,
            desde: Instant::now(),
            deshacer: false,
        });
    }
}

/// Lo que se busca de una captura, plegado: su nombre, su fecha («3 oct»,
/// «viernes») y el texto que tiene dentro si ya se leyo.
fn coincide(e: &Estado, i: usize, consulta: &str, textos: &Catalogo) -> bool {
    if consulta.is_empty() {
        return true;
    }
    let entrada = &e.lista[i];
    let ms = crate::caducidad_capturas::ms_de(entrada.cuando);
    let dia = logica::dia_local(ms);
    let nombre = logica::plegar(&crate::caducidad_capturas::nombre(&entrada.ruta));
    let fecha = logica::plegar(&format!(
        "{} {}",
        crate::ventana_chat::fecha_corta(textos, ms),
        textos.t(&format!("galeria-dia-{}", logica::dia_de_la_semana(dia)))
    ));
    let dentro = e
        .leidos
        .get(&entrada.ruta)
        .map(|(_, p)| p.as_str())
        .unwrap_or("");
    logica::encaja(consulta, &[&nombre, &fecha, dentro])
}

/// Rehace la vista (filtro y busqueda) y su disposicion para un ancho de
/// rejilla. Barato: se hace antes de cada fotograma y de cada tecla.
fn preparar(e: &mut Estado, ancho_rejilla: f32, escala: f32, textos: &Catalogo) {
    e.ahora = pixpin_shell::entorno::ahora_utc_ms();
    let consulta = logica::plegar(e.consulta.trim());
    let vista: Vec<usize> = (0..e.lista.len())
        .filter(|&i| {
            logica::pasa(e.filtro, &e.ficha(i), e.ahora) && coincide(e, i, &consulta, textos)
        })
        .collect();
    let dias: Vec<i64> = vista
        .iter()
        .map(|&i| logica::dia_local(crate::caducidad_capturas::ms_de(e.lista[i].cuando)))
        .collect();
    e.disp = logica::disponer(&dias, ancho_rejilla, escala);
    e.vista = vista;
    // Sin foco (o con uno que ya no se ve), la primera de la vista: el
    // panel de detalle nunca se queda vacio habiendo capturas.
    if e.foco_en_vista().is_none() {
        e.foco = e.vista.first().map(|&i| e.lista[i].ruta.clone());
    }
    // Las elegidas que se fueron de la carpeta dejan de estarlo.
    if !e.elegidas.is_empty() {
        let presentes: std::collections::HashSet<&PathBuf> =
            e.lista.iter().map(|x| &x.ruta).collect();
        e.elegidas.retain(|r| presentes.contains(r));
    }
}

/// Las zonas de la ventana, en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Zonas {
    rejilla: RectF,
    carril: RectF,
    panel: Option<RectF>,
}

fn zonas(w: f32, h: f32, escala: f32) -> Zonas {
    let cab = logica::CABECERA * escala;
    let carril = logica::CARRIL * escala;
    let con_panel = w >= logica::ANCHO_CON_PANEL * escala;
    let panel = if con_panel {
        logica::PANEL * escala
    } else {
        0.0
    };
    Zonas {
        carril: RectF {
            x: 0.0,
            y: cab,
            ancho: carril,
            alto: (h - cab).max(0.0),
        },
        rejilla: RectF {
            x: carril,
            y: cab,
            ancho: (w - carril - panel).max(0.0),
            alto: (h - cab).max(0.0),
        },
        panel: con_panel.then_some(RectF {
            x: w - panel,
            y: cab,
            ancho: panel,
            alto: (h - cab).max(0.0),
        }),
    }
}

fn bucle(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(monitor.area_trabajo, 1280, 820, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("galeria-titulo"))
        .context("no se pudo abrir la galeria")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la galeria")?;
    ventana.mostrar();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    ABIERTA.store(hwnd, Ordering::SeqCst);

    let dir = carpeta(ubicacion);
    let cache = ubicacion.raiz().join("cache").join("galeria");
    let (pedir, hechas) = lanzar_lector(cache.clone(), hwnd);
    let (pedir_texto, leidos) = lanzar_lector_de_texto(cache.clone(), hwnd);
    // Lo caducado se va antes de ensenar nada: no tiene que salir una
    // captura cuya fecha ya paso solo porque el barrendero aun no paso.
    let raiz = ubicacion.raiz();
    crate::caducidad_capturas::barrer(raiz, pixpin_shell::entorno::ahora_utc_ms());
    let mut e = Estado::nuevo(
        listar(&dir),
        crate::caducidad_capturas::leer(raiz, pixpin_shell::entorno::ahora_utc_ms()),
    );
    e.en_papelera = contar_papelera(raiz);
    let mut pedidas_texto: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    pedir_textos(&e, &pedir_texto, &mut pedidas_texto);
    tracing::info!(capturas = e.lista.len(), "galeria de capturas abierta");
    let mut fecha_carpeta = fecha_de(&dir);
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    let z = zonas(w, h, escala);
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = e.moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    e.botones.raton = local(p, marco);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    match e.botones.bajo_el_raton() {
                        Some(Accion::Mover) => {
                            e.escribiendo = false;
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => {
                            preparar(&mut e, z.rejilla.ancho, escala, textos);
                            let doble = e
                                .ultimo_clic
                                .is_some_and(|(b, t)| b == a && t.elapsed() < TECLA_DOBLE);
                            e.ultimo_clic = Some((a, Instant::now()));
                            hacer(&mut e, a, doble, textos, ubicacion, &mut vivo);
                        }
                        None => e.escribiendo = false,
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => {
                    let sobre_panel = z.panel.is_some_and(|r| dentro(r, e.botones.raton));
                    if !sobre_panel {
                        e.scroll = (e.scroll
                            - m as f32 * (logica::CELDA_ALTO + logica::HUECO) * escala * 0.6)
                            .clamp(0.0, e.disp.scroll_maximo(z.rejilla.alto));
                    }
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    preparar(&mut e, z.rejilla.ancho, escala, textos);
                    tecla(&mut e, vk, ctrl, z, escala, textos, ubicacion, &mut vivo);
                }
                EventoOverlay::Caracter(c) => caracter(&mut e, c),
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        // Las miniaturas que llegaron del hilo.
        while let Ok((ruta, mini, medidas)) = hechas.try_recv() {
            if let Some(m) = medidas {
                e.medidas.insert(ruta.clone(), m);
            }
            let m = match mini {
                Some(imagen) => Mini::Lista {
                    bitmap: None,
                    imagen,
                    visto: e.fotograma,
                },
                None => Mini::Imposible,
            };
            e.minis.insert(ruta, m);
            pintar = true;
        }
        // Y los textos.
        while let Ok(l) = leidos.try_recv() {
            match l {
                Leido::SinOcr => e.ocr = Some(false),
                Leido::Texto(ruta, t) => {
                    e.ocr = Some(true);
                    let plegado = logica::plegar(&t);
                    e.leidos.insert(ruta, (t, plegado));
                }
            }
            pintar = true;
        }

        // La carpeta, si cambio (una captura nueva, una borrada a mano).
        if mirado.elapsed() >= Duration::from_secs(2) {
            mirado = Instant::now();
            let ahora = fecha_de(&dir);
            if ahora != fecha_carpeta {
                fecha_carpeta = ahora;
                e.lista = listar(&dir);
                e.registro =
                    crate::caducidad_capturas::leer(raiz, pixpin_shell::entorno::ahora_utc_ms());
                pedir_textos(&e, &pedir_texto, &mut pedidas_texto);
                pintar = true;
            }
        }
        if let Some(a) = &e.aviso {
            let dura = if a.deshacer { 8_000 } else { 2_500 };
            if a.desde.elapsed() > Duration::from_millis(dura) {
                e.aviso = None;
                pintar = true;
            }
        }

        if pintar {
            e.fotograma += 1;
            preparar(&mut e, z.rejilla.ancho, escala, textos);
            e.scroll = e.scroll.clamp(0.0, e.disp.scroll_maximo(z.rejilla.alto));
            // El foco se lee antes que las demas: es lo que ensena el panel.
            if let Some(i) = e.foco_i()
                && !e.leidos.contains_key(&e.lista[i].ruta)
                && tiene_miniatura(&e.lista[i].ruta)
            {
                let _ = pedir_texto.send(e.lista[i].clone());
            }
            pedir_y_subir(&mut e, z, &pedir, &cache, &motor);
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, w, h, escala, textos)
                });
                let _ = superficie.presentar();
            }
            soltar_lejanas(&mut e, z);
            pintar = false;
        }
        let espera = if e.aviso.is_some() || e.escribiendo {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    tracing::info!("galeria de capturas cerrada");
    Ok(())
}

/// Pide el texto de las que aun no se pidieron, de la mas vieja a la mas
/// nueva: el hilo atiende primero lo ultimo, asi que empieza por las nuevas.
fn pedir_textos(
    e: &Estado,
    pedir: &Sender<Entrada>,
    pedidas: &mut std::collections::HashSet<PathBuf>,
) {
    if e.ocr == Some(false) {
        return;
    }
    for x in e.lista.iter().rev() {
        if tiene_miniatura(&x.ruta) && pedidas.insert(x.ruta.clone()) {
            let _ = pedir.send(x.clone());
        }
    }
}

fn contar_papelera(raiz: &Path) -> usize {
    std::fs::read_dir(raiz.join("papelera").join("capturas"))
        .map(|d| d.flatten().filter(|x| es_captura(&x.path())).count())
        .unwrap_or(0)
}

fn fecha_de(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// Pide al hilo las miniaturas a la vista que faltan y sube a la GPU las
/// que ya llegaron. Se hace ANTES de pintar: crear bitmaps a medio dibujo
/// no se puede. La del foco tambien, que sale en el panel.
fn pedir_y_subir(
    e: &mut Estado,
    z: Zonas,
    pedir: &Sender<(Entrada, PathBuf)>,
    cache: &Path,
    motor: &pixpin_render::MotorRender,
) {
    let margen = (logica::CELDA_ALTO + logica::HUECO) * 1.5;
    let rango = e
        .disp
        .visibles(e.scroll - margen, e.scroll + z.rejilla.alto + margen);
    let mut quiero: Vec<usize> = e.vista[rango].iter().rev().copied().collect();
    if let Some(f) = e.foco_i() {
        quiero.push(f);
    }
    // Al reves: el hilo atiende primero lo ultimo pedido, y asi lo de arriba
    // de la vista llega antes.
    for i in quiero {
        let entrada = &e.lista[i];
        if !tiene_miniatura(&entrada.ruta) {
            continue;
        }
        match e.minis.get_mut(&entrada.ruta) {
            None => {
                let en_cache = ruta_en_cache(cache, entrada);
                if pedir.send((entrada.clone(), en_cache)).is_ok() {
                    e.minis.insert(entrada.ruta.clone(), Mini::Pedida);
                }
            }
            Some(Mini::Lista {
                bitmap,
                imagen,
                visto,
            }) => {
                *visto = e.fotograma;
                if bitmap.is_none() {
                    *bitmap = motor
                        .bitmap_desde_pixeles(imagen.ancho, imagen.alto, &imagen.pixeles)
                        .ok();
                }
            }
            Some(_) => {}
        }
    }
}

/// Pasado el tope, suelta las miniaturas que hace mas que no se ven. Las
/// pedidas e imposibles no pesan y se quedan.
fn soltar_lejanas(e: &mut Estado, z: Zonas) {
    let listas = e
        .minis
        .values()
        .filter(|m| matches!(m, Mini::Lista { .. }))
        .count();
    if listas <= TOPE_EN_MEMORIA {
        return;
    }
    let margen = (logica::CELDA_ALTO + logica::HUECO) * 2.0;
    let rango = e
        .disp
        .visibles(e.scroll - margen, e.scroll + z.rejilla.alto + margen);
    let mut a_la_vista: std::collections::HashSet<&PathBuf> =
        e.vista[rango].iter().map(|&i| &e.lista[i].ruta).collect();
    if let Some(f) = &e.foco {
        a_la_vista.insert(f);
    }
    let mut viejas: Vec<(u64, PathBuf)> = e
        .minis
        .iter()
        .filter_map(|(ruta, m)| match m {
            Mini::Lista { visto, .. } if !a_la_vista.contains(ruta) => Some((*visto, ruta.clone())),
            _ => None,
        })
        .collect();
    viejas.sort_by_key(|(v, _)| *v);
    for (_, ruta) in viejas.into_iter().take(listas - TOPE_EN_MEMORIA) {
        e.minis.remove(&ruta);
    }
}

// ---------------------------------------------------------------- teclado

/// Las teclas de la galeria. Solo valen con ella delante (son de su
/// ventana, no globales), y no chocan con nada: antes solo tenia Esc.
#[allow(clippy::too_many_arguments)] // estado, tecla, ctrl, zonas, escala, textos, datos, vivo
fn tecla(
    e: &mut Estado,
    vk: u32,
    ctrl: bool,
    z: Zonas,
    escala: f32,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    vivo: &mut bool,
) {
    const ESC: u32 = 0x1B;
    const RETROCESO: u32 = 0x08;
    const INTRO: u32 = 0x0D;
    const ESPACIO: u32 = 0x20;
    const SUPR: u32 = 0x2E;
    const INICIO: u32 = 0x24;
    const FIN: u32 = 0x23;
    if ctrl && vk == u32::from(b'F') {
        e.escribiendo = true;
        return;
    }
    if e.escribiendo {
        match vk {
            ESC if !e.consulta.is_empty() => {
                e.consulta.clear();
                e.scroll = 0.0;
            }
            ESC | INTRO | 0x28 => e.escribiendo = false,
            RETROCESO if ctrl => {
                e.consulta.clear();
                e.scroll = 0.0;
            }
            RETROCESO => {
                e.consulta.pop();
                e.scroll = 0.0;
            }
            _ => {}
        }
        return;
    }
    let mover_a = |e: &mut Estado, k: usize| {
        if let Some(&i) = e.vista.get(k) {
            e.foco = Some(e.lista[i].ruta.clone());
            e.scroll = e
                .disp
                .scroll_para_ver(k, e.scroll, z.rejilla.alto, escala)
                .clamp(0.0, e.disp.scroll_maximo(z.rejilla.alto));
        }
    };
    let flecha = match vk {
        0x25 => Some(logica::Flecha::Izquierda),
        0x26 => Some(logica::Flecha::Arriba),
        0x27 => Some(logica::Flecha::Derecha),
        0x28 => Some(logica::Flecha::Abajo),
        _ => None,
    };
    if let Some(f) = flecha {
        let k = e
            .foco_en_vista()
            .map_or(0, |k| logica::mover(&e.disp, k, f));
        mover_a(e, k);
        return;
    }
    let mut vale = |a: Accion, e: &mut Estado| hacer(e, a, false, textos, ubicacion, vivo);
    match vk {
        ESC if e.eligiendo => vale(Accion::SalirEligiendo, e),
        ESC if !e.consulta.is_empty() => {
            e.consulta.clear();
            e.scroll = 0.0;
        }
        ESC => vale(Accion::Cerrar, e),
        INICIO => mover_a(e, 0),
        FIN => mover_a(e, e.vista.len().saturating_sub(1)),
        ESPACIO if !ctrl => {
            if let Some(i) = e.foco_i() {
                vale(Accion::Marca(i), e);
            }
        }
        INTRO => vale(Accion::BarraPinear, e),
        SUPR => vale(Accion::BarraBorrar, e),
        _ if ctrl && vk == u32::from(b'C') => vale(Accion::BarraCopiar, e),
        _ if ctrl && vk == u32::from(b'S') => vale(Accion::BarraConservar, e),
        _ if ctrl && vk == u32::from(b'A') => vale(Accion::ElegirTodas, e),
        _ if ctrl && vk == u32::from(b'Z') => vale(Accion::Deshacer, e),
        _ => {}
    }
}

/// Una letra escrita. Con el buscador sin foco, la primera letra lo enfoca
/// (menos el espacio, que elige).
fn caracter(e: &mut Estado, c: char) {
    if c.is_control() {
        return;
    }
    if !e.escribiendo {
        if c == ' ' {
            return;
        }
        e.escribiendo = true;
    }
    e.consulta.push(c);
    e.scroll = 0.0;
}

// --------------------------------------------------------------- acciones

fn fallo(textos: &Catalogo, motivo: String) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("motivo", motivo);
    textos.t_args("galeria-no-se-pudo", &args)
}

fn con_cuantas(textos: &Catalogo, clave: &str, n: usize) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuantas", n);
    textos.t_args(clave, &args)
}

fn hacer(
    e: &mut Estado,
    a: Accion,
    doble: bool,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    vivo: &mut bool,
) {
    let raiz = ubicacion.raiz();
    if a != Accion::Buscar && a != Accion::LimpiarBusqueda {
        e.escribiendo = false;
    }
    match a {
        Accion::Nada | Accion::Mover => {}
        Accion::Cerrar => *vivo = false,
        Accion::Carpeta | Accion::Papelera => {
            let dir = if a == Accion::Carpeta {
                carpeta(ubicacion)
            } else {
                raiz.join("papelera").join("capturas")
            };
            let _ = std::fs::create_dir_all(&dir);
            if let Err(err) = pixpin_shell::abrir(&dir) {
                e.avisar(fallo(textos, err.to_string()));
            }
        }
        Accion::Buscar => e.escribiendo = true,
        Accion::LimpiarBusqueda => {
            e.consulta.clear();
            e.escribiendo = true;
            e.scroll = 0.0;
        }
        Accion::Filtro(f) => {
            e.filtro = f;
            e.scroll = 0.0;
        }
        Accion::AlternarEligiendo => {
            e.eligiendo = !e.eligiendo;
            if !e.eligiendo {
                e.elegidas.clear();
            }
        }
        Accion::SalirEligiendo => {
            e.eligiendo = false;
            e.elegidas.clear();
        }
        Accion::ElegirDia(dia) => {
            e.eligiendo = true;
            let del_dia: Vec<PathBuf> = e
                .vista
                .iter()
                .filter(|&&i| {
                    logica::dia_local(crate::caducidad_capturas::ms_de(e.lista[i].cuando)) == dia
                })
                .map(|&i| e.lista[i].ruta.clone())
                .collect();
            // Si ya estaban todas, el mismo enlace las suelta.
            if del_dia.iter().all(|r| e.elegidas.contains(r)) {
                for r in &del_dia {
                    e.elegidas.remove(r);
                }
            } else {
                e.elegidas.extend(del_dia);
            }
        }
        Accion::ElegirTodas => {
            e.eligiendo = true;
            let todas: Vec<PathBuf> = e.vista.iter().map(|&i| e.lista[i].ruta.clone()).collect();
            if todas.iter().all(|r| e.elegidas.contains(r)) {
                e.elegidas.clear();
            } else {
                e.elegidas.extend(todas);
            }
        }
        Accion::Celda(i) => {
            let Some(r) = e.lista.get(i).map(|x| x.ruta.clone()) else {
                return;
            };
            if e.eligiendo || pixpin_shell::entrada::modificadores_pulsados().ctrl {
                e.foco = Some(r);
                hacer(e, Accion::Marca(i), false, textos, ubicacion, vivo);
            } else if doble {
                hacer(e, Accion::VerGrande(i), true, textos, ubicacion, vivo);
            } else {
                e.foco = Some(r);
            }
        }
        Accion::Marca(i) => {
            if let Some(r) = e.lista.get(i).map(|x| x.ruta.clone()) {
                e.eligiendo = true;
                if !e.elegidas.remove(&r) {
                    e.elegidas.insert(r);
                }
            }
        }
        Accion::VerGrande(i) => {
            // Doble clic: abrirla con el visor de Windows.
            if doble && let Some(r) = e.lista.get(i).map(|x| x.ruta.clone()) {
                if let Err(err) = pixpin_shell::abrir(&r) {
                    e.avisar(fallo(textos, err.to_string()));
                }
            }
        }
        Accion::Pinear(i) => pinear(e, &[i], textos),
        Accion::Copiar(i) => copiar(e, &[i], textos),
        Accion::Conservar(i) => conservar_varias(e, &[i], raiz, textos),
        Accion::BorrarUna(i) => borrar_varias(e, &[i], raiz, textos),
        Accion::Prorrogar(i) => {
            if let Some(x) = e.lista.get(i).cloned() {
                match crate::caducidad_capturas::prorrogar(
                    raiz,
                    &x,
                    pixpin_shell::entorno::ahora_utc_ms(),
                ) {
                    Ok(reg) => {
                        e.registro = reg;
                        e.avisar(textos.t("galeria-prorrogada"));
                    }
                    Err(err) => e.avisar(fallo(textos, err.to_string())),
                }
            }
        }
        Accion::CopiarTexto(i) => {
            if let Some((t, _)) = e.lista.get(i).and_then(|x| e.leidos.get(&x.ruta)) {
                let aviso = match pixpin_codec::copiar_texto(t) {
                    Ok(()) => textos.t("galeria-texto-copiado"),
                    Err(err) => fallo(textos, err.to_string()),
                };
                e.avisar(aviso);
            }
        }
        Accion::BarraConservar => {
            let v = e.objetivos();
            conservar_varias(e, &v, raiz, textos);
        }
        Accion::BarraPinear => {
            let v = e.objetivos();
            pinear(e, &v, textos);
        }
        Accion::BarraCopiar => {
            let v = e.objetivos();
            copiar(e, &v, textos);
        }
        Accion::BarraBorrar => {
            let v = e.objetivos();
            borrar_varias(e, &v, raiz, textos);
        }
        Accion::Deshacer => {
            if e.deshacer.is_empty() {
                return;
            }
            let borradas = std::mem::take(&mut e.deshacer);
            let (vueltas, reg) = devolver(raiz, &borradas);
            if let Some(reg) = reg {
                e.registro = reg;
            }
            e.lista = listar(&carpeta(ubicacion));
            e.en_papelera = contar_papelera(raiz);
            if let Some(b) = borradas.first() {
                e.foco = Some(b.original.clone());
            }
            e.avisar(con_cuantas(textos, "galeria-recuperadas", vueltas));
        }
    }
}

fn pinear(e: &mut Estado, v: &[usize], textos: &Catalogo) {
    let rutas: Vec<PathBuf> = v
        .iter()
        .filter_map(|&i| e.lista.get(i))
        .map(|x| x.ruta.clone())
        .collect();
    if rutas.is_empty() {
        return;
    }
    // Por el mismo camino que «Abrir con PixPin»: la ventana principal, que
    // es la que tiene los pines, las pinea.
    let ok = pixpin_shell::mensajero::enviar_ficheros(&rutas);
    e.avisar(if !ok {
        fallo(textos, "PixPin".into())
    } else if rutas.len() == 1 {
        textos.t("galeria-pineada")
    } else {
        con_cuantas(textos, "galeria-pineadas", rutas.len())
    });
}

fn copiar(e: &mut Estado, v: &[usize], textos: &Catalogo) {
    let rutas: Vec<PathBuf> = v
        .iter()
        .filter_map(|&i| e.lista.get(i))
        .map(|x| x.ruta.clone())
        .collect();
    let hecho = match rutas.as_slice() {
        [] => return,
        // Una imagen sola, como imagen: se pega en cualquier sitio.
        [r] if tiene_miniatura(r) => pixpin_codec::imagen::cargar(r)
            .map_err(|err| err.to_string())
            .and_then(|img| pixpin_codec::copiar_imagen(&img).map_err(|err| err.to_string())),
        // Varias (o un video), como ficheros: lo que pega el Explorador.
        _ => pixpin_codec::copiar_ficheros(&rutas).map_err(|err| err.to_string()),
    };
    e.avisar(match hecho {
        Ok(()) if rutas.len() == 1 => textos.t("galeria-copiada"),
        Ok(()) => con_cuantas(textos, "galeria-copiadas", rutas.len()),
        Err(m) => fallo(textos, m),
    });
}

fn conservar_varias(e: &mut Estado, v: &[usize], raiz: &Path, textos: &Catalogo) {
    let pendientes: Vec<PathBuf> = v
        .iter()
        .filter(|&&i| i < e.lista.len() && e.se_va(i).is_some())
        .map(|&i| e.lista[i].ruta.clone())
        .collect();
    if pendientes.is_empty() {
        return;
    }
    let mut hechas = 0;
    let mut error = None;
    for r in &pendientes {
        match conservar(raiz, r) {
            Ok(reg) => {
                e.registro = reg;
                hechas += 1;
            }
            Err(err) => error = Some(err.to_string()),
        }
    }
    e.avisar(match error {
        Some(m) => fallo(textos, m),
        None if hechas == 1 => textos.t("galeria-conservada-aviso"),
        None => con_cuantas(textos, "galeria-conservadas-aviso", hechas),
    });
}

fn borrar_varias(e: &mut Estado, v: &[usize], raiz: &Path, textos: &Catalogo) {
    let rutas: Vec<PathBuf> = v
        .iter()
        .filter_map(|&i| e.lista.get(i))
        .map(|x| x.ruta.clone())
        .collect();
    if rutas.is_empty() {
        return;
    }
    // El foco pasa a la siguiente de la vista que no se borra.
    let siguiente = e.foco_en_vista().and_then(|k| {
        e.vista[k..]
            .iter()
            .chain(e.vista[..k].iter().rev())
            .map(|&i| &e.lista[i].ruta)
            .find(|r| !rutas.contains(r))
            .cloned()
    });
    let (borradas, reg, error) = llevar_a_la_papelera(raiz, &rutas);
    if let Some(reg) = reg {
        e.registro = reg;
    }
    for b in &borradas {
        e.lista.retain(|x| x.ruta != b.original);
        e.minis.remove(&b.original);
        e.elegidas.remove(&b.original);
    }
    if e.foco
        .as_ref()
        .is_some_and(|f| borradas.iter().any(|b| &b.original == f))
    {
        e.foco = siguiente;
    }
    e.en_papelera = contar_papelera(raiz);
    if let Some(m) = error {
        e.avisar(fallo(textos, m));
        e.deshacer = borradas;
        return;
    }
    let n = borradas.len();
    e.deshacer = borradas;
    e.aviso = Some(Aviso {
        texto: if n == 1 {
            textos.t("galeria-borrada")
        } else {
            con_cuantas(textos, "galeria-borradas", n)
        },
        desde: Instant::now(),
        deshacer: true,
    });
}

/// **Borra varias de una vez**, con lo que hace falta para deshacerlo: a la
/// papelera de PixPin, y se olvida si estaban conservadas o prorrogadas (su
/// nombre queda libre). Lo que se pudo mover se devuelve aunque alguna
/// fallara; el error, aparte.
pub(crate) fn llevar_a_la_papelera(
    raiz: &Path,
    rutas: &[PathBuf],
) -> (
    Vec<Borrada>,
    Option<crate::caducidad_capturas::Registro>,
    Option<String>,
) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let antes = crate::caducidad_capturas::leer(raiz, ahora);
    let mut borradas = Vec::new();
    let mut error = None;
    for r in rutas {
        match a_la_papelera(raiz, r) {
            Ok(destino) => {
                tracing::info!(ruta = %r.display(), destino = %destino.display(), "captura a la papelera");
                let n = crate::caducidad_capturas::nombre(r);
                borradas.push(Borrada {
                    original: r.clone(),
                    destino,
                    conservada: antes.conservadas.contains(&n),
                    prorroga: antes.prorrogadas.get(&n).copied(),
                });
            }
            Err(err) => error = Some(err.to_string()),
        }
    }
    let tocar = borradas
        .iter()
        .any(|b| b.conservada || b.prorroga.is_some());
    let reg = if tocar {
        crate::caducidad_capturas::cambiar(raiz, ahora, |reg| {
            for b in &borradas {
                let n = crate::caducidad_capturas::nombre(&b.original);
                reg.conservadas.remove(&n);
                reg.prorrogadas.remove(&n);
            }
        })
        .map_err(|e| tracing::warn!(?e, "no se pudo olvidar la captura conservada"))
        .ok()
    } else {
        None
    };
    (borradas, reg, error)
}

/// **Deshace un borrado**: cada una vuelve a su sitio (si su nombre sigue
/// libre) con lo que tenia apuntado. Devuelve cuantas volvieron.
pub(crate) fn devolver(
    raiz: &Path,
    borradas: &[Borrada],
) -> (usize, Option<crate::caducidad_capturas::Registro>) {
    let mut vueltas = Vec::new();
    for b in borradas {
        if b.original.exists() {
            tracing::warn!(ruta = %b.original.display(), "no se devuelve: ya hay otra con ese nombre");
            continue;
        }
        match std::fs::rename(&b.destino, &b.original) {
            Ok(()) => vueltas.push(b),
            Err(e) => tracing::warn!(?e, "no se pudo devolver la captura"),
        }
    }
    let tocar = vueltas.iter().any(|b| b.conservada || b.prorroga.is_some());
    let reg = if tocar {
        crate::caducidad_capturas::cambiar(raiz, pixpin_shell::entorno::ahora_utc_ms(), |reg| {
            for b in &vueltas {
                let n = crate::caducidad_capturas::nombre(&b.original);
                if b.conservada {
                    reg.conservadas.insert(n.clone());
                }
                if let Some(t) = b.prorroga {
                    reg.prorrogadas.insert(n, t);
                }
            }
        })
        .ok()
    } else {
        None
    };
    (vueltas.len(), reg)
}

/// **Conserva una captura**: entra en «Mensajes guardados» como una foto
/// mas (por `meter_en_proyecto`, con su sello y su cerrojo, y de ahi viaja
/// al movil) y se apunta para que no caduque.
pub(crate) fn conservar(
    raiz: &Path,
    ruta: &Path,
) -> std::io::Result<crate::caducidad_capturas::Registro> {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let nombre_equipo = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into());
    let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, &nombre_equipo)
        .map(|i| i.yo.codigo())
        .unwrap_or_default();
    let ficha = pixpin_proyecto::almacen::asegurar_guardados(raiz, ahora, &aparato)?;
    let bytes = std::fs::read(ruta)?;
    let nombre = crate::caducidad_capturas::nombre(ruta);
    let hechos = crate::ventana_chat::meter_en_proyecto(
        raiz,
        &ficha.id,
        &[(nombre.clone(), bytes)],
        &aparato,
    )?;
    if hechos.is_empty() {
        return Err(std::io::Error::other("no entro en el chat"));
    }
    crate::ventana_chat::refrescar();
    tracing::info!(ruta = %ruta.display(), "captura conservada en Mensajes guardados");
    crate::caducidad_capturas::cambiar(raiz, ahora, |reg| {
        reg.conservadas.insert(nombre);
    })
}

// ---------------------------------------------------------------- pintar

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}

/// Lo comun de dos recuadros, si lo hay.
fn cortar(a: RectF, b: RectF) -> Option<RectF> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.ancho).min(b.x + b.ancho);
    let y1 = (a.y + a.alto).min(b.y + b.alto);
    (x1 > x0 && y1 > y0).then_some(RectF {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    })
}

fn tramo_negrita(t: &str) -> [Tramo; 1] {
    [Tramo {
        inicio: 0,
        longitud: t.encode_utf16().count() as u32,
        estilo: EstiloTexto {
            negrita: true,
            ..Default::default()
        },
    }]
}

fn negrita(p: &Pintor, t: &str, x: f32, y: f32, tam: f32, color: Color) {
    p.parrafo(t, x, y, tam, 10_000.0, &tramo_negrita(t), color);
}

fn medir_negrita(p: &Pintor, t: &str, tam: f32) -> (f32, f32) {
    p.medir_parrafo(t, tam, 10_000.0, &tramo_negrita(t))
}

/// Un recuadro redondeado con borde: el borde es el mismo recuadro un
/// poco mayor por debajo (no hay trazo redondeado).
fn con_borde(p: &Pintor, r: RectF, radio: f32, fondo: Color, borde: Color, grosor: f32) {
    p.rellenar_redondeado(r, radio, borde);
    p.rellenar_redondeado(encoger(r, grosor), (radio - grosor).max(0.0), fondo);
}

/// La chapita de un atajo («Ctrl C», «Esc»), centrada en vertical en `yc`.
/// Devuelve su ancho.
fn chapita(p: &Pintor, t: &str, x: f32, yc: f32, escala: f32, sobre_azul: bool) -> f32 {
    let tam = 11.0 * escala;
    let (tw, _) = p.medir_texto(t, tam);
    let caja = RectF {
        x,
        y: yc - 9.0 * escala,
        ancho: tw + 12.0 * escala,
        alto: 18.0 * escala,
    };
    if sobre_azul {
        p.rellenar_redondeado(caja, 5.0 * escala, blanco(0.2));
    } else {
        con_borde(
            p,
            caja,
            5.0 * escala,
            hex(0x2E2E31),
            blanco(0.14),
            1.0 * escala,
        );
    }
    let (_, th) = p.medir_texto(t, tam);
    p.texto(
        t,
        caja.x + 6.0 * escala,
        yc - th / 2.0,
        tam,
        if sobre_azul {
            Color::BLANCO
        } else {
            hex(0xD1D1D6)
        },
    );
    caja.ancho
}

fn ancho_chapita(p: &Pintor, t: &str, escala: f32) -> f32 {
    p.medir_texto(t, 11.0 * escala).0 + 12.0 * escala
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estilo {
    Normal,
    Primario,
    Verde,
    /// Rojo y con borde, sin relleno: «Borrar».
    Rojo,
    /// Encendido (el modo de elegir).
    Activo,
    /// Sin nada que hacer: atenuado y sin zona.
    Apagado,
}

/// Lo que mide un boton con icono, rotulo y chapita.
fn ancho_boton(p: &Pintor, rotulo: &str, icono: bool, chapa: Option<&str>, escala: f32) -> f32 {
    let mut w = 28.0 * escala;
    if !rotulo.is_empty() {
        w += p.medir_texto(rotulo, 14.0 * escala).0;
    }
    if icono {
        w += 16.0 * escala + if rotulo.is_empty() { 0.0 } else { 8.0 * escala };
    }
    if let Some(c) = chapa {
        w += 8.0 * escala + ancho_chapita(p, c, escala);
    }
    w
}

/// **Un boton de la v2**: alto de 40, icono de 16, texto de 14, y la
/// chapita de su atajo si la tiene. Lo apunta para el raton (salvo
/// apagado).
#[allow(clippy::too_many_arguments)] // estado, pintor, caja, accion, rotulo, icono, estilo, chapita, escala
fn boton(
    e: &mut Estado,
    p: &Pintor,
    caja: RectF,
    a: Accion,
    rotulo: &str,
    icono: Option<&Icono>,
    estilo: Estilo,
    chapa: Option<&str>,
    escala: f32,
) {
    let encima = estilo != Estilo::Apagado && dentro(caja, e.botones.raton);
    let radio = 10.0 * escala;
    let (fondo, tinta) = match estilo {
        Estilo::Normal => (blanco(if encima { 0.15 } else { 0.09 }), CLARO),
        Estilo::Primario => (if encima { hex(0x1A73F0) } else { AZUL_PRI }, Color::BLANCO),
        Estilo::Verde => (if encima { hex(0x2E9E49) } else { VERDE_V }, Color::BLANCO),
        // Opaco: el borde es un recuadro mayor por debajo, y un relleno
        // transparente lo dejaria ver entero (saldria rojo macizo).
        Estilo::Rojo => (
            mezcla(hex(0x2A2A2D), ROJO_TEXTO, if encima { 0.14 } else { 0.0 }),
            ROJO_TEXTO,
        ),
        Estilo::Activo => (alfa(AZUL_V, if encima { 0.3 } else { 0.2 }), Color::BLANCO),
        Estilo::Apagado => (blanco(0.05), blanco(0.35)),
    };
    match estilo {
        Estilo::Rojo => con_borde(p, caja, radio, fondo, alfa(ROJO_TEXTO, 0.4), 1.0 * escala),
        Estilo::Activo => con_borde(p, caja, radio, hex(0x1B2B40), AZUL_V, 1.0 * escala),
        _ => p.rellenar_redondeado(caja, radio, fondo),
    }
    if estilo == Estilo::Activo && encima {
        p.rellenar_redondeado(encoger(caja, 1.0 * escala), radio, fondo);
    }
    let tam = 14.0 * escala;
    let (tw, th) = if rotulo.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(rotulo, tam)
    };
    let lado = 16.0 * escala;
    let hueco = if rotulo.is_empty() || icono.is_none() {
        0.0
    } else {
        8.0 * escala
    };
    let chapa_w = chapa.map_or(0.0, |c| 8.0 * escala + ancho_chapita(p, c, escala));
    let total = icono.map_or(0.0, |_| lado) + hueco + tw + chapa_w;
    let mut x = caja.x + (caja.ancho - total) / 2.0;
    let yc = caja.y + caja.alto / 2.0;
    if let Some(i) = icono {
        p.icono(
            i,
            RectF {
                x,
                y: yc - lado / 2.0,
                ancho: lado,
                alto: lado,
            },
            tinta,
        );
        x += lado + hueco;
    }
    if !rotulo.is_empty() {
        p.texto(rotulo, x, yc - th / 2.0, tam, tinta);
        x += tw;
    }
    if let Some(c) = chapa {
        chapita(
            p,
            c,
            x + 8.0 * escala,
            yc,
            escala,
            matches!(estilo, Estilo::Primario | Estilo::Verde),
        );
    }
    if estilo != Estilo::Apagado {
        e.botones.zona(caja, a);
    }
}

/// Una marca de «hecho» dibujada con dos trazos (no hay icono de check
/// suelto en `material`).
fn pintar_check(p: &Pintor, c: (f32, f32), lado: f32, color: Color, grosor: f32) {
    let (x, y) = (c.0 - lado / 2.0, c.1 - lado / 2.0);
    p.linea(
        (x + lado * 0.2, y + lado * 0.52),
        (x + lado * 0.42, y + lado * 0.72),
        grosor,
        color,
    );
    p.linea(
        (x + lado * 0.42, y + lado * 0.72),
        (x + lado * 0.8, y + lado * 0.3),
        grosor,
        color,
    );
}

/// El dibujito de cada filtro del carril.
fn glifo_filtro(p: &Pintor, f: Filtro, caja: RectF, color: Color, escala: f32) {
    let g = 1.8 * escala;
    let (cx, cy) = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    let s = caja.ancho;
    match f {
        Filtro::Todas => {
            let l = s * 0.36;
            for (dx, dy) in [(0.1, 0.1), (0.54, 0.1), (0.1, 0.54), (0.54, 0.54)] {
                p.trazar(
                    RectF {
                        x: caja.x + s * dx,
                        y: caja.y + s * dy,
                        ancho: l,
                        alto: l,
                    },
                    g,
                    color,
                );
            }
        }
        Filtro::Hoy => {
            p.anillo((cx, cy), s * 0.2, g, color);
            for k in 0..8 {
                let ang = k as f32 * std::f32::consts::FRAC_PI_4;
                let (sn, cs) = ang.sin_cos();
                p.linea(
                    (cx + cs * s * 0.33, cy + sn * s * 0.33),
                    (cx + cs * s * 0.45, cy + sn * s * 0.45),
                    g,
                    color,
                );
            }
        }
        Filtro::Semana => {
            let r = RectF {
                x: caja.x + s * 0.1,
                y: caja.y + s * 0.2,
                ancho: s * 0.8,
                alto: s * 0.7,
            };
            p.trazar(r, g, color);
            p.linea(
                (r.x, r.y + s * 0.22),
                (r.x + r.ancho, r.y + s * 0.22),
                g,
                color,
            );
            p.linea(
                (caja.x + s * 0.32, caja.y + s * 0.1),
                (caja.x + s * 0.32, caja.y + s * 0.28),
                g,
                color,
            );
            p.linea(
                (caja.x + s * 0.68, caja.y + s * 0.1),
                (caja.x + s * 0.68, caja.y + s * 0.28),
                g,
                color,
            );
        }
        Filtro::Conservadas => p.icono(&mi::BOOKMARK_BORDER, caja, color),
        Filtro::Pronto => p.icono(&mi::ALARM, caja, color),
        Filtro::Gif => {
            let t = "GIF";
            let tam = 8.5 * escala;
            let (tw, th) = medir_negrita(p, t, tam);
            p.rellenar_redondeado(encoger(caja, 1.0 * escala), 3.0 * escala, alfa(color, 0.25));
            negrita(p, t, cx - tw / 2.0, cy - th / 2.0, tam, color);
        }
        Filtro::Videos => p.icono(&mi::PLAY_ARROW, caja, color),
        Filtro::ConTexto => {
            let t = "Aa";
            let tam = 12.0 * escala;
            let (tw, th) = medir_negrita(p, t, tam);
            negrita(p, t, cx - tw / 2.0, cy - th / 2.0, tam, color);
        }
    }
}

fn pintar_todo(e: &mut Estado, p: &Pintor, w: f32, h: f32, escala: f32, textos: &Catalogo) {
    p.limpiar(FONDO_V);
    e.botones.vaciar();
    let z = zonas(w, h, escala);
    pintar_rejilla(e, p, z, escala, textos);
    pintar_carril(e, p, z, escala, textos);
    if let Some(panel) = z.panel {
        pintar_panel(e, p, panel, escala, textos);
    }
    pintar_cabecera(e, p, w, escala, textos);
}

fn pintar_cabecera(e: &mut Estado, p: &Pintor, w: f32, escala: f32, textos: &Catalogo) {
    let alto = logica::CABECERA * escala;
    let caja = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto,
    };
    p.rellenar(caja, FONDO_V);
    p.rellenar(
        RectF {
            x: 0.0,
            y: alto - 1.0 * escala,
            ancho: w,
            alto: 1.0 * escala,
        },
        blanco(0.07),
    );
    // Su zona de mover, antes que los botones (que le ganan).
    e.botones.zona(caja, Accion::Mover);
    let x0 = 20.0 * escala;
    negrita(
        p,
        &textos.t("galeria-titulo"),
        x0,
        12.0 * escala,
        17.0 * escala,
        CLARO,
    );
    p.texto_linea(
        &con_cuantas(textos, "galeria-subtitulo", e.lista.len()),
        x0,
        36.0 * escala,
        12.0 * escala,
        180.0 * escala,
        GRIS,
    );

    // A la derecha, de fuera adentro: cerrar, la carpeta y elegir.
    let lado = 40.0 * escala;
    let yb = (alto - lado) / 2.0;
    let cerrar = RectF {
        x: w - 16.0 * escala - lado,
        y: yb,
        ancho: lado,
        alto: lado,
    };
    boton(
        e,
        p,
        cerrar,
        Accion::Cerrar,
        "",
        Some(&mi::CLOSE),
        Estilo::Normal,
        None,
        escala,
    );
    let rotulo = textos.t("galeria-abrir-carpeta");
    let wc = ancho_boton(p, &rotulo, true, None, escala);
    let carpeta_b = RectF {
        x: cerrar.x - 10.0 * escala - wc,
        y: yb,
        ancho: wc,
        alto: lado,
    };
    boton(
        e,
        p,
        carpeta_b,
        Accion::Carpeta,
        &rotulo,
        Some(&mi::FOLDER),
        Estilo::Normal,
        None,
        escala,
    );
    let (rotulo, estilo, chapa) = if e.eligiendo {
        (textos.t("galeria-eligiendo"), Estilo::Activo, Some("Esc"))
    } else {
        (textos.t("galeria-elegir"), Estilo::Normal, None)
    };
    let we = ancho_boton(p, &rotulo, true, chapa, escala);
    let elegir = RectF {
        x: carpeta_b.x - 10.0 * escala - we,
        y: yb,
        ancho: we,
        alto: lado,
    };
    boton(
        e,
        p,
        elegir,
        Accion::AlternarEligiendo,
        &rotulo,
        Some(&mi::CHECK_BOX),
        estilo,
        chapa,
        escala,
    );

    // El buscador, entre el titulo y los botones.
    let xb = logica::CARRIL * escala + 4.0 * escala;
    let ancho_b = (elegir.x - 16.0 * escala - xb).min(500.0 * escala);
    if ancho_b < 120.0 * escala {
        return;
    }
    let caja_b = RectF {
        x: xb,
        y: yb,
        ancho: ancho_b,
        alto: lado,
    };
    let borde = if e.escribiendo { AZUL_V } else { blanco(0.1) };
    con_borde(p, caja_b, 10.0 * escala, hex(0x2A2A2D), borde, 1.0 * escala);
    e.botones.zona(caja_b, Accion::Buscar);
    let li = 16.0 * escala;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: caja_b.x + 12.0 * escala,
            y: caja_b.y + (lado - li) / 2.0,
            ancho: li,
            alto: li,
        },
        GRIS,
    );
    let tx = caja_b.x + 36.0 * escala;
    let tam = 14.0 * escala;
    // A la derecha, la chapita del atajo o, con algo escrito, la equis.
    let derecha = if e.consulta.is_empty() {
        let cw = ancho_chapita(p, "Ctrl F", escala);
        chapita(
            p,
            "Ctrl F",
            caja_b.x + caja_b.ancho - 12.0 * escala - cw,
            caja_b.y + lado / 2.0,
            escala,
            false,
        );
        cw + 20.0 * escala
    } else {
        let l = 32.0 * escala;
        let x = RectF {
            x: caja_b.x + caja_b.ancho - 4.0 * escala - l,
            y: caja_b.y + (lado - l) / 2.0,
            ancho: l,
            alto: l,
        };
        if dentro(x, e.botones.raton) {
            p.rellenar_redondeado(x, 8.0 * escala, blanco(0.1));
        }
        p.icono(&mi::CLOSE, encoger(x, 8.0 * escala), GRIS);
        e.botones.zona(x, Accion::LimpiarBusqueda);
        l + 8.0 * escala
    };
    let ancho_t = (caja_b.x + caja_b.ancho - derecha - tx).max(0.0);
    let (_, th) = p.medir_texto("Ag", tam);
    let ty = caja_b.y + (lado - th) / 2.0;
    if e.consulta.is_empty() {
        let pista = if e.ocr == Some(false) {
            textos.t("galeria-buscar-sin-ocr")
        } else {
            textos.t("galeria-buscar")
        };
        p.texto_linea(&pista, tx, ty, tam, ancho_t, GRIS);
    } else {
        p.texto_linea(&e.consulta, tx, ty, tam, ancho_t, CLARO);
    }
    if e.escribiendo {
        let (cw, _) = if e.consulta.is_empty() {
            (0.0, 0.0)
        } else {
            p.medir_texto(&e.consulta, tam)
        };
        let cx = (tx + cw + 1.0 * escala).min(tx + ancho_t);
        p.rellenar(
            RectF {
                x: cx,
                y: ty,
                ancho: 1.5 * escala,
                alto: th,
            },
            AZUL_V,
        );
    }
}

fn pintar_carril(e: &mut Estado, p: &Pintor, z: Zonas, escala: f32, textos: &Catalogo) {
    let c = z.carril;
    p.rellenar(c, LATERAL);
    p.rellenar(
        RectF {
            x: c.x + c.ancho - 1.0 * escala,
            y: c.y,
            ancho: 1.0 * escala,
            alto: c.alto,
        },
        blanco(0.06),
    );
    e.botones.zona(c, Accion::Nada);
    let x = c.x + 10.0 * escala;
    let ancho = c.ancho - 20.0 * escala;
    let fila = 44.0 * escala;
    let mut y = c.y + 12.0 * escala;
    let cuenta = |f: Filtro| {
        (0..e.lista.len())
            .filter(|&i| logica::pasa(f, &e.ficha(i), e.ahora))
            .count()
    };
    let cuentas: Vec<(Filtro, usize)> = Filtro::CARRIL
        .iter()
        .filter(|&&f| f != Filtro::ConTexto || e.ocr != Some(false))
        .map(|&f| (f, cuenta(f)))
        .collect();
    let encima = e.botones.raton;
    for (f, n) in cuentas {
        if f == Filtro::Gif {
            p.rellenar(
                RectF {
                    x: x + 6.0 * escala,
                    y: y + 7.0 * escala,
                    ancho: ancho - 12.0 * escala,
                    alto: 1.0 * escala,
                },
                blanco(0.07),
            );
            y += 16.0 * escala;
        }
        let caja = RectF {
            x,
            y,
            ancho,
            alto: fila,
        };
        let elegido = e.filtro == f;
        if elegido {
            p.rellenar_redondeado(caja, 10.0 * escala, alfa(AZUL_V, 0.18));
        } else if dentro(caja, encima) {
            p.rellenar_redondeado(caja, 10.0 * escala, blanco(0.05));
        }
        let tinta_icono = match (elegido, f) {
            (true, _) => AZUL_V,
            (false, Filtro::Pronto) => NARANJA,
            _ => GRIS,
        };
        let li = 18.0 * escala;
        glifo_filtro(
            p,
            f,
            RectF {
                x: x + 12.0 * escala,
                y: y + (fila - li) / 2.0,
                ancho: li,
                alto: li,
            },
            tinta_icono,
            escala,
        );
        let tam = 14.0 * escala;
        let (_, th) = p.medir_texto("Ag", tam);
        let num = n.to_string();
        let (nw, nh) = p.medir_texto(&num, 13.0 * escala);
        p.texto_linea(
            &textos.t(f.clave()),
            x + 40.0 * escala,
            y + (fila - th) / 2.0,
            tam,
            ancho - 60.0 * escala - nw,
            if elegido {
                Color::BLANCO
            } else {
                hex(0xE5E5EA)
            },
        );
        let color_n = if f == Filtro::Pronto && n > 0 {
            NARANJA
        } else {
            GRIS
        };
        p.texto(
            &num,
            x + ancho - 12.0 * escala - nw,
            y + (fila - nh) / 2.0,
            13.0 * escala,
            color_n,
        );
        e.botones.zona(caja, Accion::Filtro(f));
        y += fila + 2.0 * escala;
    }

    // Abajo: la papelera, lo que ocupan y la ayuda de teclas.
    let mut yb = c.y + c.alto - 12.0 * escala;
    let total: u64 = e.lista.iter().map(|x| x.bytes).sum();
    let mut args = fluent_bundle::FluentArgs::new();
    args.set(
        "tamano",
        logica::tamano_legible(total, pixpin_shell::entorno::separador_decimal()),
    );
    let ocupan = textos.t_args("galeria-ocupan", &args);
    let (_, oh) = p.medir_texto(&ocupan, 12.0 * escala);
    yb -= oh;
    p.texto(&ocupan, x + 12.0 * escala, yb, 12.0 * escala, GRIS);
    yb -= 4.0 * escala + fila;
    let pap = RectF {
        x,
        y: yb,
        ancho,
        alto: fila,
    };
    if yb > y + 8.0 * escala {
        if dentro(pap, encima) {
            p.rellenar_redondeado(pap, 10.0 * escala, blanco(0.05));
        }
        let li = 18.0 * escala;
        p.icono(
            &mi::DELETE,
            RectF {
                x: x + 12.0 * escala,
                y: yb + (fila - li) / 2.0,
                ancho: li,
                alto: li,
            },
            GRIS,
        );
        let tam = 14.0 * escala;
        let (_, th) = p.medir_texto("Ag", tam);
        p.texto(
            &textos.t("galeria-papelera"),
            x + 40.0 * escala,
            yb + (fila - th) / 2.0,
            tam,
            hex(0xE5E5EA),
        );
        let num = e.en_papelera.to_string();
        let (nw, nh) = p.medir_texto(&num, 13.0 * escala);
        p.texto(
            &num,
            x + ancho - 12.0 * escala - nw,
            yb + (fila - nh) / 2.0,
            13.0 * escala,
            GRIS,
        );
        e.botones.zona(pap, Accion::Papelera);
        p.rellenar(
            RectF {
                x: x + 6.0 * escala,
                y: yb - 6.0 * escala,
                ancho: ancho - 12.0 * escala,
                alto: 1.0 * escala,
            },
            blanco(0.07),
        );
    }
    // La ayuda de las teclas, si cabe.
    let tam = 12.0 * escala;
    let linea = 22.0 * escala;
    let y_ayuda = yb - 14.0 * escala - 2.0 * linea;
    if y_ayuda > y + 8.0 * escala {
        let mut xa = x + 10.0 * escala;
        let yc = y_ayuda + linea / 2.0;
        xa += chapita(p, "← → ↑ ↓", xa, yc, escala, false) + 6.0 * escala;
        p.texto(
            &textos.t("galeria-ayuda-mover"),
            xa,
            yc - p.medir_texto("Ag", tam).1 / 2.0,
            tam,
            GRIS,
        );
        let mut xa = x + 10.0 * escala;
        let yc = yc + linea;
        let th = p.medir_texto("Ag", tam).1;
        xa += chapita(
            p,
            textos.t("galeria-tecla-espacio").as_str(),
            xa,
            yc,
            escala,
            false,
        ) + 6.0 * escala;
        let t = textos.t("galeria-ayuda-elegir");
        p.texto(&t, xa, yc - th / 2.0, tam, GRIS);
        xa += p.medir_texto(&t, tam).0 + 6.0 * escala;
        xa += chapita(p, "Enter", xa, yc, escala, false) + 6.0 * escala;
        p.texto(
            &textos.t("galeria-ayuda-pinear"),
            xa,
            yc - th / 2.0,
            tam,
            GRIS,
        );
    }
}

/// Que pone la pastilla de caducidad y de que color va.
fn pastilla(textos: &Catalogo, se_va: Option<i64>, ahora: i64) -> (String, Color, Color, Tono) {
    let plazo = logica::plazo(se_va, ahora);
    let texto = match plazo {
        Plazo::Conservada => textos.t("galeria-conservada"),
        Plazo::Hoy => textos.t("galeria-se-borra-hoy"),
        Plazo::Manana => textos.t("galeria-se-borra-manana"),
        Plazo::EnDias(n) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("dias", n);
            textos.t_args("galeria-se-borra-en", &args)
        }
        Plazo::ElDia(t) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("fecha", crate::ventana_chat::fecha_corta(textos, t));
            textos.t_args("galeria-se-borra", &args)
        }
    };
    let tono = logica::tono(plazo);
    let (fondo, tinta) = match tono {
        Tono::Lejos => (
            Color {
                a: 0.85,
                ..hex(0x141416)
            },
            hex(0xE5E5EA),
        ),
        Tono::Pronto => (NARANJA, TINTA_OSCURA),
        Tono::Urgente => (ROJO_V, Color::BLANCO),
        Tono::Conservada => (VERDE_V, Color::BLANCO),
    };
    (texto, fondo, tinta, tono)
}

/// El nombre del dia de un grupo: «Hoy», «Ayer» o «Viernes».
fn titulo_del_dia(textos: &Catalogo, dia: i64, hoy: i64) -> (String, String) {
    let semana = textos.t(&format!("galeria-dia-{}", logica::dia_de_la_semana(dia)));
    let fecha = crate::ventana_chat::fecha_corta(
        textos,
        dia * logica::DIA_MS + logica::DIA_MS / 2 - pixpin_shell::entorno::desfase_local_ms(),
    );
    match hoy - dia {
        0 => (textos.t("galeria-hoy"), format!("{semana} {fecha}")),
        1 => (textos.t("galeria-ayer"), format!("{semana} {fecha}")),
        _ => {
            let mut s = semana.clone();
            if let Some(p) = s.get_mut(0..1) {
                p.make_ascii_uppercase();
            }
            (s, fecha)
        }
    }
}

fn pintar_rejilla(e: &mut Estado, p: &Pintor, z: Zonas, escala: f32, textos: &Catalogo) {
    let r = z.rejilla;
    if e.vista.is_empty() {
        let t = if e.lista.is_empty() {
            textos.t("galeria-vacia")
        } else if !e.consulta.trim().is_empty() {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("texto", e.consulta.trim().to_string());
            textos.t_args("galeria-sin-resultados", &args)
        } else {
            textos.t("galeria-filtro-vacio")
        };
        let tam = 15.0 * escala;
        let ancho = r.ancho - 64.0 * escala;
        let (tw, th) = p.medir_texto_ajustado(&t, tam, ancho);
        p.texto_ajustado(
            &t,
            r.x + (r.ancho - tw) / 2.0,
            r.y + (r.alto - th) / 2.0,
            tam,
            ancho,
            GRIS,
        );
    }
    let encima = e.botones.raton;
    let hoy = logica::dia_local(e.ahora);
    let (ox, oy) = (r.x, r.y - e.scroll);
    let grupos = e.disp.grupos.clone();
    let rango = e.disp.visibles(e.scroll - 4.0, e.scroll + r.alto + 4.0);
    p.con_recorte(r, |p| {
        // Los titulos de los dias que asoman.
        for g in &grupos {
            let y = oy + g.y;
            if y + logica::TITULO_DIA * escala < r.y || y > r.y + r.alto {
                continue;
            }
            let (titulo, sub) = titulo_del_dia(textos, g.dia, hoy);
            let x = ox + logica::LADO_REJILLA * escala;
            let tam = 14.0 * escala;
            let (tw, th) = medir_negrita(p, &titulo, tam);
            let yt = y + (logica::TITULO_DIA * escala - th) / 2.0;
            negrita(p, &titulo, x, yt, tam, CLARO);
            let sub = format!("{sub} · {}", g.hasta - g.desde);
            p.texto(
                &sub,
                x + tw + 10.0 * escala,
                yt + 1.0 * escala,
                13.0 * escala,
                GRIS,
            );
            let enlace = textos.t("galeria-elegir-dia");
            let (ew, eh) = p.medir_texto(&enlace, 13.0 * escala);
            let ze = RectF {
                x: r.x + r.ancho - logica::LADO_REJILLA * escala - ew - 8.0 * escala,
                y: y + (logica::TITULO_DIA * escala - 32.0 * escala) / 2.0,
                ancho: ew + 8.0 * escala,
                alto: 32.0 * escala,
            };
            let color = if dentro(ze, encima) {
                hex(0xA0E4FF)
            } else {
                ENLACE
            };
            p.texto(
                &enlace,
                ze.x + 4.0 * escala,
                ze.y + (ze.alto - eh) / 2.0,
                13.0 * escala,
                color,
            );
            if let Some(zz) = cortar(ze, r) {
                e.botones.zona(zz, Accion::ElegirDia(g.dia));
            }
        }
        for k in rango {
            let i = e.vista[k];
            let rc = e.disp.celdas[k];
            let c = RectF {
                x: ox + rc.x,
                y: oy + rc.y,
                ..rc
            };
            pintar_celda(e, p, c, r, i, escala, textos);
        }
    });
    if e.eligiendo {
        pintar_barra_elegidas(e, p, r, escala, textos);
    }
    pintar_aviso(e, p, r, escala, textos);
}

#[allow(clippy::too_many_arguments)] // estado, pintor, celda, zona, indice, escala, textos
fn pintar_celda(
    e: &mut Estado,
    p: &Pintor,
    c: RectF,
    zona: RectF,
    i: usize,
    escala: f32,
    textos: &Catalogo,
) {
    let entrada = e.lista[i].clone();
    let encima = dentro(c, e.botones.raton) && dentro(zona, e.botones.raton);
    let elegida = e.elegidas.contains(&entrada.ruta);
    let enfocada = e.foco.as_ref() == Some(&entrada.ruta);
    let radio = 12.0 * escala;
    // El anillo del foco y de elegida, por fuera de la celda.
    if enfocada {
        p.rellenar_redondeado(encoger(c, -6.0 * escala), radio + 6.0 * escala, CLARO);
        p.rellenar_redondeado(encoger(c, -3.0 * escala), radio + 3.0 * escala, AZUL_V);
    } else if elegida {
        p.rellenar_redondeado(encoger(c, -3.0 * escala), radio + 3.0 * escala, AZUL_V);
    } else if encima {
        p.rellenar_redondeado(encoger(c, -escala), radio + 1.0 * escala, blanco(0.35));
    }
    p.rellenar_redondeado(c, radio, hex(0x2A2A2E));
    match e.minis.get(&entrada.ruta) {
        Some(Mini::Lista {
            bitmap: Some(b),
            imagen,
            ..
        }) => {
            // Un poco hacia dentro: las esquinas redondeadas se notan sin
            // recortar el mapa de bits.
            crate::miniaturas::pintar_recortado(
                p,
                b,
                encoger(c, 2.0 * escala),
                imagen.ancho,
                imagen.alto,
            )
        }
        _ => {
            let lado = 40.0 * escala;
            let icono = if tiene_miniatura(&entrada.ruta) {
                &mi::IMAGE
            } else {
                &mi::PLAY_ARROW
            };
            p.icono(
                icono,
                RectF {
                    x: c.x + (c.ancho - lado) / 2.0,
                    y: c.y + (c.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                APAGADO,
            );
        }
    }
    if let Some(zc) = cortar(c, zona) {
        e.botones.zona(zc, Accion::Celda(i));
    }

    // Arriba a la derecha, la caducidad.
    let (texto, fondo, tinta, tono) = pastilla(textos, e.se_va(i), e.ahora);
    let tam = 12.0 * escala;
    let (tw, th) = p.medir_texto(&texto, tam);
    let con_check = tono == Tono::Conservada;
    let extra = if con_check { 17.0 * escala } else { 0.0 };
    let ancho = (tw + 16.0 * escala + extra).min(c.ancho - 50.0 * escala);
    let bd = RectF {
        x: c.x + c.ancho - 8.0 * escala - ancho,
        y: c.y + 8.0 * escala,
        ancho,
        alto: th + 6.0 * escala,
    };
    p.rellenar_redondeado(bd, 7.0 * escala, fondo);
    if con_check {
        pintar_check(
            p,
            (bd.x + 8.0 * escala + 6.0 * escala, bd.y + bd.alto / 2.0),
            12.0 * escala,
            tinta,
            2.0 * escala,
        );
    }
    p.texto_linea(
        &texto,
        bd.x + 8.0 * escala + extra,
        bd.y + 3.0 * escala,
        tam,
        bd.ancho - 16.0 * escala - extra,
        tinta,
    );

    // Abajo a la izquierda, lo que es: texto, GIF o video.
    let ext = entrada
        .ruta
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let chip = if ext == "mp4" {
        Some(textos.t("galeria-chip-video"))
    } else if ext == "gif" {
        Some("GIF".to_string())
    } else if e
        .leidos
        .get(&entrada.ruta)
        .is_some_and(|(t, _)| !t.trim().is_empty())
    {
        Some(textos.t("galeria-chip-texto"))
    } else {
        None
    };
    let botones_encima = encima && !e.eligiendo;
    if let Some(chip) = chip
        && !botones_encima
    {
        let (cw, ch) = p.medir_texto(&chip, tam);
        let cc = RectF {
            x: c.x + 8.0 * escala,
            y: c.y + c.alto - 8.0 * escala - ch - 6.0 * escala,
            ancho: cw + 16.0 * escala,
            alto: ch + 6.0 * escala,
        };
        p.rellenar_redondeado(
            cc,
            7.0 * escala,
            Color {
                a: 0.85,
                ..hex(0x141416)
            },
        );
        p.texto(
            &chip,
            cc.x + 8.0 * escala,
            cc.y + 3.0 * escala,
            tam,
            hex(0xE5E5EA),
        );
    }

    // Con el raton encima: Pinear, Copiar y Conservar.
    if botones_encima && c.y >= zona.y && c.y + c.alto <= zona.y + zona.alto {
        let lado = 40.0 * escala;
        let hueco = 6.0 * escala;
        let y = c.y + c.alto - 8.0 * escala - lado;
        let conservada = e.se_va(i).is_none();
        let n_iconos = if conservada { 1.0 } else { 2.0 };
        let ancho_pin = c.ancho - 16.0 * escala - n_iconos * (lado + hueco);
        let mut x = c.x + 8.0 * escala;
        boton(
            e,
            p,
            RectF {
                x,
                y,
                ancho: ancho_pin,
                alto: lado,
            },
            Accion::Pinear(i),
            &textos.t("galeria-pinear"),
            Some(&mi::PUSH_PIN),
            Estilo::Primario,
            None,
            escala,
        );
        x += ancho_pin + hueco;
        let caja = RectF {
            x,
            y,
            ancho: lado,
            alto: lado,
        };
        p.rellenar_redondeado(
            caja,
            10.0 * escala,
            Color {
                a: 0.9,
                ..hex(0x1C1C1E)
            },
        );
        boton(
            e,
            p,
            caja,
            Accion::Copiar(i),
            "",
            Some(&mi::CONTENT_COPY),
            Estilo::Normal,
            None,
            escala,
        );
        x += lado + hueco;
        if !conservada {
            let caja = RectF {
                x,
                y,
                ancho: lado,
                alto: lado,
            };
            boton(
                e,
                p,
                caja,
                Accion::Conservar(i),
                "",
                Some(&mi::BOOKMARK_ADD),
                Estilo::Verde,
                None,
                escala,
            );
            // La pista del atajo, sobre el boton que esta bajo el raton.
            if dentro(caja, e.botones.raton) {
                pista(p, &textos.t("galeria-conservar"), "Ctrl S", caja, escala);
            }
        }
    }

    // Arriba a la izquierda, la marca de elegir (al elegir, o al pasar).
    if e.eligiendo || encima {
        let d = 26.0 * escala;
        let ck = RectF {
            x: c.x + 8.0 * escala,
            y: c.y + 8.0 * escala,
            ancho: d,
            alto: d,
        };
        let centro = (ck.x + d / 2.0, ck.y + d / 2.0);
        if elegida {
            p.circulo(centro, d / 2.0, AZUL_V);
            pintar_check(p, centro, 16.0 * escala, Color::BLANCO, 2.5 * escala);
        } else {
            p.circulo(
                centro,
                d / 2.0,
                Color {
                    a: 0.3,
                    ..Color::NEGRO
                },
            );
            let borde = if dentro(ck, e.botones.raton) {
                Color::BLANCO
            } else {
                blanco(0.85)
            };
            p.anillo(centro, d / 2.0 - 1.0 * escala, 2.0 * escala, borde);
        }
        if let Some(zc) = cortar(encoger(ck, -4.0 * escala), zona) {
            e.botones.zona(zc, Accion::Marca(i));
        }
    }
}

/// Un globito con el nombre de una accion y su atajo, encima de `caja`.
fn pista(p: &Pintor, t: &str, atajo: &str, caja: RectF, escala: f32) {
    let tam = 12.0 * escala;
    let (tw, th) = p.medir_texto(t, tam);
    let cw = ancho_chapita(p, atajo, escala);
    let ancho = tw + 6.0 * escala + cw + 16.0 * escala;
    let alto = th.max(18.0 * escala) + 8.0 * escala;
    let g = RectF {
        x: caja.x + caja.ancho - ancho,
        y: caja.y - 8.0 * escala - alto,
        ancho,
        alto,
    };
    p.rellenar_redondeado(g, 7.0 * escala, hex(0x3A3A3C));
    p.texto(t, g.x + 8.0 * escala, g.y + (alto - th) / 2.0, tam, CLARO);
    chapita(
        p,
        atajo,
        g.x + 8.0 * escala + tw + 6.0 * escala,
        g.y + alto / 2.0,
        escala,
        false,
    );
}

fn pintar_barra_elegidas(e: &mut Estado, p: &Pintor, r: RectF, escala: f32, textos: &Catalogo) {
    let alto = 60.0 * escala;
    let barra = RectF {
        x: r.x + 20.0 * escala,
        y: r.y + r.alto - 16.0 * escala - alto,
        ancho: r.ancho - 40.0 * escala,
        alto,
    };
    p.rellenar_redondeado(
        encoger(barra, -8.0 * escala),
        20.0 * escala,
        Color {
            a: 0.35,
            ..Color::NEGRO
        },
    );
    con_borde(
        p,
        barra,
        14.0 * escala,
        BARRA_ELEGIDAS,
        blanco(0.12),
        1.0 * escala,
    );
    e.botones.zona(barra, Accion::Nada);
    let n = e.elegidas.len();
    let hay = n > 0;
    let lado = 40.0 * escala;
    let yb = barra.y + (alto - lado) / 2.0;
    // De derecha a izquierda: salir, borrar (apartado), copiar, pinear,
    // conservar.
    let mut x = barra.x + barra.ancho - 10.0 * escala - lado;
    let salir = RectF {
        x,
        y: yb,
        ancho: lado,
        alto: lado,
    };
    if dentro(salir, e.botones.raton) {
        p.rellenar_redondeado(salir, 10.0 * escala, blanco(0.08));
    }
    p.icono(&mi::CLOSE, encoger(salir, 11.0 * escala), hex(0xC7C7CC));
    e.botones.zona(salir, Accion::SalirEligiendo);
    let estilo = |s: Estilo| if hay { s } else { Estilo::Apagado };
    let acciones = [
        (
            Accion::BarraBorrar,
            textos.t("galeria-borrar"),
            &mi::DELETE,
            estilo(Estilo::Rojo),
        ),
        (
            Accion::BarraCopiar,
            textos.t("galeria-copiar"),
            &mi::CONTENT_COPY,
            estilo(Estilo::Normal),
        ),
        (
            Accion::BarraPinear,
            textos.t("galeria-pinear"),
            &mi::PUSH_PIN,
            estilo(Estilo::Normal),
        ),
        (
            Accion::BarraConservar,
            textos.t("galeria-conservar"),
            &mi::BOOKMARK_ADD,
            estilo(Estilo::Verde),
        ),
    ];
    for (k, (a, rotulo, icono, s)) in acciones.iter().enumerate() {
        let wb = ancho_boton(p, rotulo, true, None, escala) - 4.0 * escala;
        x -= 8.0 * escala + wb;
        boton(
            e,
            p,
            RectF {
                x,
                y: yb,
                ancho: wb,
                alto: lado,
            },
            *a,
            rotulo,
            Some(icono),
            *s,
            None,
            escala,
        );
        if k == 0 {
            // La raya que aparta «Borrar» de lo demas.
            x -= 9.0 * escala;
            p.rellenar(
                RectF {
                    x: x + 4.0 * escala,
                    y: barra.y + (alto - 28.0 * escala) / 2.0,
                    ancho: 1.0 * escala,
                    alto: 28.0 * escala,
                },
                blanco(0.14),
            );
        }
    }
    // A la izquierda, cuantas y «Todas».
    let xi = barra.x + 16.0 * escala;
    let cuantas = con_cuantas(textos, "galeria-elegidas", n);
    let tam = 15.0 * escala;
    let (cw, ch) = medir_negrita(p, &cuantas, tam);
    if xi + cw < x - 8.0 * escala {
        negrita(p, &cuantas, xi, barra.y + (alto - ch) / 2.0, tam, CLARO);
    }
    let todas = textos.t("galeria-elegir-todas");
    let (tw, th) = p.medir_texto(&todas, 13.0 * escala);
    let chw = ancho_chapita(p, "Ctrl A", escala);
    let xt = xi + cw + 10.0 * escala;
    let con_chapa = xt + tw + 6.0 * escala + chw < x - 8.0 * escala;
    if xt + tw < x - 8.0 * escala {
        let zt = RectF {
            x: xt - 4.0 * escala,
            y: yb,
            ancho: tw + 8.0 * escala + if con_chapa { 6.0 * escala + chw } else { 0.0 },
            alto: lado,
        };
        let color = if dentro(zt, e.botones.raton) {
            hex(0xA0E4FF)
        } else {
            ENLACE
        };
        p.texto(
            &todas,
            xt,
            barra.y + (alto - th) / 2.0,
            13.0 * escala,
            color,
        );
        if con_chapa {
            chapita(
                p,
                "Ctrl A",
                xt + tw + 6.0 * escala,
                barra.y + alto / 2.0,
                escala,
                false,
            );
        }
        e.botones.zona(zt, Accion::ElegirTodas);
    }
}

fn pintar_aviso(e: &mut Estado, p: &Pintor, r: RectF, escala: f32, textos: &Catalogo) {
    let Some(aviso) = &e.aviso else {
        return;
    };
    let texto = aviso.texto.clone();
    let con_deshacer = aviso.deshacer && !e.deshacer.is_empty();
    let tam = 14.0 * escala;
    let (tw, th) = p.medir_texto(&texto, tam);
    let rotulo = textos.t("galeria-deshacer");
    let extra = if con_deshacer {
        16.0 * escala
            + p.medir_texto(&rotulo, tam).0
            + 8.0 * escala
            + ancho_chapita(p, "Ctrl Z", escala)
    } else {
        0.0
    };
    let alto = th.max(18.0 * escala) + 20.0 * escala;
    let ancho = (tw + extra + 32.0 * escala).min(r.ancho - 32.0 * escala);
    let abajo = if e.eligiendo {
        16.0 + 60.0 + 12.0
    } else {
        24.0
    } * escala;
    let caja = RectF {
        x: r.x + (r.ancho - ancho) / 2.0,
        y: r.y + r.alto - abajo - alto,
        ancho,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        10.0 * escala,
        Color {
            a: 0.94,
            ..hex(0x3A3A3C)
        },
    );
    e.botones.zona(caja, Accion::Nada);
    p.texto_linea(
        &texto,
        caja.x + 16.0 * escala,
        caja.y + (alto - th) / 2.0,
        tam,
        ancho - extra - 32.0 * escala,
        CLARO,
    );
    if con_deshacer {
        let x = caja.x + 16.0 * escala + tw + 16.0 * escala;
        let (rw, _) = p.medir_texto(&rotulo, tam);
        let zona = RectF {
            x: x - 6.0 * escala,
            y: caja.y,
            ancho: caja.x + caja.ancho - x,
            alto,
        };
        let color = if dentro(zona, e.botones.raton) {
            hex(0xA0E4FF)
        } else {
            ENLACE
        };
        negrita(p, &rotulo, x, caja.y + (alto - th) / 2.0, tam, color);
        chapita(
            p,
            "Ctrl Z",
            x + rw + 8.0 * escala,
            caja.y + alto / 2.0,
            escala,
            false,
        );
        e.botones.zona(zona, Accion::Deshacer);
    }
}

/// «Hoy, 10:42», «Ayer, 10:42», «3 oct, 10:42».
fn fecha_y_hora(textos: &Catalogo, ms: i64, ahora: i64) -> String {
    let local = pixpin_shell::entorno::a_local(ms);
    let minutos = local.rem_euclid(logica::DIA_MS) / 60_000;
    let hora = format!("{}:{:02}", minutos / 60, minutos % 60);
    let dia = match logica::dia_local(ahora) - logica::dia_local(ms) {
        0 => textos.t("galeria-hoy"),
        1 => textos.t("galeria-ayer"),
        _ => crate::ventana_chat::fecha_corta(textos, ms),
    };
    format!("{dia}, {hora}")
}

fn pintar_panel(e: &mut Estado, p: &Pintor, panel: RectF, escala: f32, textos: &Catalogo) {
    p.rellenar(panel, LATERAL);
    p.rellenar(
        RectF {
            x: panel.x,
            y: panel.y,
            ancho: 1.0 * escala,
            alto: panel.alto,
        },
        blanco(0.06),
    );
    e.botones.zona(panel, Accion::Nada);
    let pad = 16.0 * escala;
    let x = panel.x + pad;
    let ancho = panel.ancho - 2.0 * pad;
    let Some(i) = e.foco_i().filter(|i| e.vista.contains(i)) else {
        let t = textos.t("galeria-detalle-vacio");
        let tam = 14.0 * escala;
        let (tw, th) = p.medir_texto_ajustado(&t, tam, ancho);
        p.texto_ajustado(
            &t,
            x + (ancho - tw) / 2.0,
            panel.y + (panel.alto - th) / 2.0,
            tam,
            ancho,
            GRIS,
        );
        return;
    };
    let entrada = e.lista[i].clone();
    let mut y = panel.y + pad;

    // La vista previa, entera (sin recortar), sobre fondo oscuro.
    let previa = RectF {
        x,
        y,
        ancho,
        alto: 170.0 * escala,
    };
    p.rellenar_redondeado(previa, 12.0 * escala, FONDO_V);
    match e.minis.get(&entrada.ruta) {
        Some(Mini::Lista {
            bitmap: Some(b),
            imagen,
            ..
        }) => {
            let (iw, ih) = (imagen.ancho as f32, imagen.alto as f32);
            let f = (previa.ancho / iw).min(previa.alto / ih);
            let (dw, dh) = (iw * f, ih * f);
            p.bitmap_con(
                b,
                RectF {
                    x: previa.x + (previa.ancho - dw) / 2.0,
                    y: previa.y + (previa.alto - dh) / 2.0,
                    ancho: dw,
                    alto: dh,
                },
                None,
                pixpin_render::Interpolacion::Lineal,
            );
        }
        _ => {
            let lado = 48.0 * escala;
            let icono = if tiene_miniatura(&entrada.ruta) {
                &mi::IMAGE
            } else {
                &mi::PLAY_ARROW
            };
            p.icono(
                icono,
                RectF {
                    x: previa.x + (previa.ancho - lado) / 2.0,
                    y: previa.y + (previa.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                APAGADO,
            );
        }
    }
    let ver = textos.t("galeria-ver-grande");
    let (vw, vh) = p.medir_texto(&ver, 12.0 * escala);
    let cv = RectF {
        x: previa.x + previa.ancho - 8.0 * escala - vw - 16.0 * escala,
        y: previa.y + previa.alto - 8.0 * escala - vh - 6.0 * escala,
        ancho: vw + 16.0 * escala,
        alto: vh + 6.0 * escala,
    };
    p.rellenar_redondeado(
        cv,
        7.0 * escala,
        Color {
            a: 0.85,
            ..hex(0x141416)
        },
    );
    p.texto(
        &ver,
        cv.x + 8.0 * escala,
        cv.y + 3.0 * escala,
        12.0 * escala,
        hex(0xE5E5EA),
    );
    e.botones.zona(previa, Accion::VerGrande(i));
    y += previa.alto + 12.0 * escala;

    // El nombre y lo que es.
    let nombre = crate::caducidad_capturas::nombre(&entrada.ruta);
    let (_, nh) = medir_negrita(p, "Ag", 16.0 * escala);
    p.con_recorte(
        RectF {
            x,
            y,
            ancho,
            alto: nh,
        },
        |p| negrita(p, &nombre, x, y, 16.0 * escala, CLARO),
    );
    y += nh + 2.0 * escala;
    let ext = entrada
        .ruta
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let tipo = textos.t(match ext.as_str() {
        "mp4" => "galeria-tipo-video",
        "gif" => "galeria-tipo-gif",
        _ => "galeria-tipo-captura",
    });
    let (_, sh) = p.medir_texto(&tipo, 13.0 * escala);
    p.texto(&tipo, x, y, 13.0 * escala, GRIS);
    y += sh + 12.0 * escala;

    // Pinear (la principal, en azul), Copiar y, apartado, Borrar.
    let lado = 40.0 * escala;
    let borrar = RectF {
        x: x + ancho - lado,
        y,
        ancho: lado,
        alto: lado,
    };
    let copiar_r = textos.t("galeria-copiar");
    let wc = ancho_boton(p, &copiar_r, false, None, escala) - 4.0 * escala;
    let copiar_b = RectF {
        x: borrar.x - 14.0 * escala - wc,
        y,
        ancho: wc,
        alto: lado,
    };
    let pinear_b = RectF {
        x,
        y,
        ancho: copiar_b.x - 8.0 * escala - x,
        alto: lado,
    };
    boton(
        e,
        p,
        pinear_b,
        Accion::Pinear(i),
        &textos.t("galeria-pinear"),
        None,
        Estilo::Primario,
        Some("Enter"),
        escala,
    );
    boton(
        e,
        p,
        copiar_b,
        Accion::Copiar(i),
        &copiar_r,
        None,
        Estilo::Normal,
        None,
        escala,
    );
    boton(
        e,
        p,
        borrar,
        Accion::BorrarUna(i),
        "",
        Some(&mi::DELETE),
        Estilo::Rojo,
        None,
        escala,
    );
    if dentro(copiar_b, e.botones.raton) {
        pista(p, &copiar_r, "Ctrl C", copiar_b, escala);
    } else if dentro(borrar, e.botones.raton) {
        pista(p, &textos.t("galeria-borrar"), "Supr", borrar, escala);
    }
    y += lado + 12.0 * escala;

    // Fecha y tamano.
    let ms = crate::caducidad_capturas::ms_de(entrada.cuando);
    let peso = logica::tamano_legible(entrada.bytes, pixpin_shell::entorno::separador_decimal());
    let tamano = match e.medidas.get(&entrada.ruta) {
        Some((mw, mh)) => format!("{mw} × {mh} · {peso}"),
        None => peso,
    };
    for (clave, valor) in [
        ("galeria-fecha", fecha_y_hora(textos, ms, e.ahora)),
        ("galeria-tamano", tamano),
    ] {
        let fila = 32.0 * escala;
        let (_, th) = p.medir_texto("Ag", 14.0 * escala);
        p.texto(
            &textos.t(clave),
            x,
            y + (fila - th) / 2.0,
            13.0 * escala,
            GRIS,
        );
        p.texto_linea(
            &valor,
            x + 104.0 * escala,
            y + (fila - th) / 2.0,
            14.0 * escala,
            ancho - 104.0 * escala,
            CLARO,
        );
        y += fila;
    }
    y += 8.0 * escala;

    // La caducidad, con su color, y lo que se puede hacer con ella.
    let se_va = e.se_va(i);
    let (texto, fondo, _, tono) = pastilla(textos, se_va, e.ahora);
    let color = match tono {
        Tono::Lejos => GRIS,
        Tono::Pronto => NARANJA,
        Tono::Urgente => ROJO_TEXTO,
        Tono::Conservada => hex(0x30D158),
    };
    let _ = fondo;
    let alto_caja = if se_va.is_some() { 108.0 } else { 70.0 } * escala;
    let caja = RectF {
        x,
        y,
        ancho,
        alto: alto_caja,
    };
    con_borde(
        p,
        caja,
        12.0 * escala,
        Color {
            a: 1.0,
            ..mezcla(LATERAL, color, 0.08)
        },
        alfa(color, 0.3),
        1.0 * escala,
    );
    let li = 16.0 * escala;
    let yi = y + 12.0 * escala;
    let icono = if se_va.is_some() {
        &mi::ALARM
    } else {
        &mi::BOOKMARK_ADD
    };
    p.icono(
        icono,
        RectF {
            x: x + 12.0 * escala,
            y: yi + 1.0 * escala,
            ancho: li,
            alto: li,
        },
        color,
    );
    let (tw, th) = medir_negrita(p, &texto, 14.0 * escala);
    negrita(p, &texto, x + 36.0 * escala, yi, 14.0 * escala, CLARO);
    let detalle = match se_va {
        Some(t) => {
            let n = logica::dias_que_faltan(t, e.ahora).max(0);
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("dias", n);
            textos.t_args("galeria-en-dias", &args)
        }
        None => textos.t("galeria-conservada-detalle"),
    };
    if se_va.is_some() {
        p.texto_linea(
            &format!("· {detalle}"),
            x + 36.0 * escala + tw + 6.0 * escala,
            yi + 1.0 * escala,
            13.0 * escala,
            (ancho - 48.0 * escala - tw).max(0.0),
            GRIS,
        );
        let yb = yi + th + 12.0 * escala;
        let mitad = (ancho - 24.0 * escala - 8.0 * escala) / 2.0;
        boton(
            e,
            p,
            RectF {
                x: x + 12.0 * escala,
                y: yb,
                ancho: mitad,
                alto: lado,
            },
            Accion::Conservar(i),
            &textos.t("galeria-conservar"),
            None,
            Estilo::Verde,
            None,
            escala,
        );
        boton(
            e,
            p,
            RectF {
                x: x + 12.0 * escala + mitad + 8.0 * escala,
                y: yb,
                ancho: mitad,
                alto: lado,
            },
            Accion::Prorrogar(i),
            &textos.t("galeria-dar-mas"),
            None,
            Estilo::Normal,
            None,
            escala,
        );
    } else {
        p.texto_ajustado(
            &detalle,
            x + 36.0 * escala,
            yi + th + 4.0 * escala,
            13.0 * escala,
            ancho - 48.0 * escala,
            GRIS,
        );
    }
    y += alto_caja + 12.0 * escala;

    // El texto reconocido.
    let resto = panel.y + panel.alto - pad - y;
    if resto < 80.0 * escala {
        return;
    }
    let leido = e
        .leidos
        .get(&entrada.ruta)
        .map(|(t, _)| t.trim().to_string());
    let (_, lh) = p.medir_texto("Ag", 13.0 * escala);
    let fila = 32.0 * escala;
    p.texto(
        &textos.t("galeria-texto-reconocido"),
        x,
        y + (fila - lh) / 2.0,
        13.0 * escala,
        GRIS,
    );
    if leido.as_ref().is_some_and(|t| !t.is_empty()) {
        let r = textos.t("galeria-copiar-texto");
        let (rw, rh) = p.medir_texto(&r, 13.0 * escala);
        let b = RectF {
            x: x + ancho - rw - 20.0 * escala,
            y,
            ancho: rw + 20.0 * escala,
            alto: fila,
        };
        p.rellenar_redondeado(
            b,
            8.0 * escala,
            blanco(if dentro(b, e.botones.raton) {
                0.15
            } else {
                0.09
            }),
        );
        p.texto(
            &r,
            b.x + 10.0 * escala,
            b.y + (fila - rh) / 2.0,
            13.0 * escala,
            CLARO,
        );
        e.botones.zona(b, Accion::CopiarTexto(i));
    }
    y += fila + 6.0 * escala;
    let caja_t = RectF {
        x,
        y,
        ancho,
        alto: (panel.y + panel.alto - pad - y).max(0.0),
    };
    con_borde(
        p,
        caja_t,
        10.0 * escala,
        FONDO_V,
        blanco(0.08),
        1.0 * escala,
    );
    let (texto, color) = match (&leido, e.ocr) {
        _ if !tiene_miniatura(&entrada.ruta) => (textos.t("galeria-texto-video"), GRIS),
        (Some(t), _) if !t.is_empty() => (t.clone(), hex(0xE5E5EA)),
        (Some(_), _) => (textos.t("galeria-texto-nada"), GRIS),
        (None, Some(false)) => (textos.t("galeria-texto-sin-ocr"), GRIS),
        (None, _) => (textos.t("galeria-texto-leyendo"), GRIS),
    };
    let dentro_t = encoger(caja_t, 10.0 * escala);
    p.con_recorte(dentro_t, |p| {
        p.texto_ajustado(
            &texto,
            dentro_t.x + 2.0 * escala,
            dentro_t.y,
            13.0 * escala,
            dentro_t.ancho - 4.0 * escala,
            color,
        )
    });
}

/// `a` con un poco (`t`) de `b` encima.
fn mezcla(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(nombre: &str, segundos: u64) -> Entrada {
        Entrada {
            ruta: PathBuf::from(format!(r"C:\datos\capturas\{nombre}")),
            cuando: SystemTime::UNIX_EPOCH + Duration::from_secs(segundos),
            bytes: 10,
        }
    }

    fn temporal(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-galeria-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn la_mas_nueva_va_primero_aunque_su_numero_sea_menor() {
        // El numero se reutiliza al borrar: `captura-0005` puede ser de hoy.
        let mut v = vec![
            entrada("captura-0190.png", 100),
            entrada("captura-0005.png", 300),
            entrada("captura-0191.png", 200),
        ];
        ordenar(&mut v);
        let nombres: Vec<_> = v
            .iter()
            .map(|e| e.ruta.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            nombres,
            ["captura-0005.png", "captura-0191.png", "captura-0190.png"]
        );
    }

    #[test]
    fn a_igual_hora_gana_el_nombre_mas_alto() {
        let mut v = vec![
            entrada("captura-0001.png", 5),
            entrada("captura-0002.png", 5),
        ];
        ordenar(&mut v);
        assert!(v[0].ruta.ends_with("captura-0002.png"));
    }

    #[test]
    fn solo_se_listan_imagenes_y_videos() {
        assert!(es_captura(Path::new("a/captura-0001.png")));
        assert!(es_captura(Path::new("a/grabacion.MP4")));
        assert!(es_captura(Path::new("a/foto.JPG")));
        // Casos negativos: lo que no es una captura no sale.
        assert!(!es_captura(Path::new("a/notas.txt")));
        assert!(!es_captura(Path::new("a/sin-extension")));
        assert!(!tiene_miniatura(Path::new("a/grabacion.mp4")));
    }

    #[test]
    fn listar_una_carpeta_real_ignora_lo_vacio_y_lo_ajeno() {
        let dir = temporal("listar");
        std::fs::write(dir.join("captura-0001.png"), b"x").unwrap();
        // Reservada y aun sin escribir: no sale todavia.
        std::fs::write(dir.join("captura-0002.png"), b"").unwrap();
        std::fs::write(dir.join("leeme.txt"), b"x").unwrap();
        let v = listar(&dir);
        assert_eq!(v.len(), 1);
        assert!(v[0].ruta.ends_with("captura-0001.png"));
        // Y una carpeta que no existe es una lista vacia, no un error.
        assert!(listar(&dir.join("no-esta")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_la_papelera_no_pisa_y_quita_de_la_carpeta() {
        let raiz = temporal("pap");
        let capturas = raiz.join("capturas");
        std::fs::create_dir_all(&capturas).unwrap();
        let a = capturas.join("captura-0001.png");
        std::fs::write(&a, b"uno").unwrap();
        let d1 = a_la_papelera(&raiz, &a).unwrap();
        assert!(!a.exists(), "se fue de la carpeta");
        assert_eq!(std::fs::read(&d1).unwrap(), b"uno");
        // Otra con el mismo nombre no pisa a la primera.
        std::fs::write(&a, b"dos").unwrap();
        let d2 = a_la_papelera(&raiz, &a).unwrap();
        assert_ne!(d1, d2);
        assert_eq!(std::fs::read(&d1).unwrap(), b"uno");
        assert_eq!(std::fs::read(&d2).unwrap(), b"dos");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn borrar_varias_y_deshacer_las_devuelve_con_lo_que_tenian() {
        let raiz = temporal("deshacer");
        let dir = carpeta_en(&raiz);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, c) = (dir.join("a.png"), dir.join("b.png"), dir.join("c.png"));
        for r in [&a, &b, &c] {
            std::fs::write(r, b"x").unwrap();
        }
        crate::caducidad_capturas::cambiar(&raiz, 1, |r| {
            r.conservadas.insert("a.png".into());
            r.prorrogadas.insert("b.png".into(), 99);
        })
        .unwrap();
        let (borradas, reg, error) = llevar_a_la_papelera(&raiz, &[a.clone(), b.clone()]);
        assert!(error.is_none());
        assert_eq!(borradas.len(), 2);
        assert!(
            !a.exists() && !b.exists() && c.exists(),
            "la que no se eligio se queda"
        );
        let reg = reg.expect("el registro cambio");
        assert!(!reg.conservadas.contains("a.png"));
        assert!(!reg.prorrogadas.contains_key("b.png"));
        // Deshacer: vuelven a su sitio, conservada y prorrogada como antes.
        let (vueltas, reg) = devolver(&raiz, &borradas);
        assert_eq!(vueltas, 2);
        assert!(a.exists() && b.exists());
        let reg = reg.unwrap();
        assert!(reg.conservadas.contains("a.png"));
        assert_eq!(reg.prorrogadas.get("b.png"), Some(&99));
        // Caso negativo: si su nombre ya lo tiene otra, no se pisa.
        let (borradas, _, _) = llevar_a_la_papelera(&raiz, std::slice::from_ref(&c));
        std::fs::write(&c, b"nueva").unwrap();
        assert_eq!(devolver(&raiz, &borradas).0, 0);
        assert_eq!(std::fs::read(&c).unwrap(), b"nueva");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_cache_cambia_si_cambia_la_captura_y_el_texto_va_aparte() {
        let cache = Path::new("c");
        let a = entrada("captura-0001.png", 10);
        let mut b = a.clone();
        b.bytes = 11;
        assert_ne!(ruta_en_cache(cache, &a), ruta_en_cache(cache, &b));
        let mut c = a.clone();
        c.cuando += Duration::from_secs(1);
        assert_ne!(ruta_en_cache(cache, &a), ruta_en_cache(cache, &c));
        // Caso negativo: el texto no pisa a la miniatura.
        assert_ne!(ruta_del_texto(cache, &a), ruta_en_cache(cache, &a));
        assert_eq!(ruta_del_texto(cache, &a).extension().unwrap(), "txt");
    }

    #[test]
    fn la_miniatura_se_reduce_y_queda_en_la_cache() {
        let dir = temporal("mini");
        let ruta = dir.join("captura-0001.png");
        let grande = ImagenRgba {
            ancho: 600,
            alto: 300,
            pixeles: [10u8, 200, 30, 255].repeat(600 * 300),
        };
        pixpin_codec::guardar(&grande, &ruta, pixpin_codec::FormatoImagen::Png).unwrap();
        let en_cache = dir.join("mini.png");
        let m = leer_reducida(&ruta, &en_cache).expect("se lee");
        assert_eq!(
            (m.ancho, m.alto),
            (LADO, LADO / 2),
            "lado mayor a LADO, sin deformar"
        );
        assert!(en_cache.exists(), "y queda guardada para la proxima vez");
        // La segunda sale de la cache aunque la captura ya no este.
        std::fs::remove_file(&ruta).unwrap();
        assert!(leer_reducida(&ruta, &en_cache).is_some());
        // Caso negativo: sin captura ni cache no hay miniatura (ni panico).
        assert!(leer_reducida(&ruta, &dir.join("otra.png")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------ estado y teclado

    /// Tres capturas de hoy (la 2 conservada) y una de hace diez dias.
    fn estado() -> Estado {
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let seg = |ms: i64| (ms / 1000) as u64;
        let lista = vec![
            entrada("captura-0003.png", seg(ahora) - 10),
            entrada("captura-0002.png", seg(ahora) - 20),
            entrada("grabacion.mp4", seg(ahora) - 30),
            entrada("captura-0001.png", seg(ahora) - 10 * 86_400),
        ];
        let registro = crate::caducidad_capturas::Registro {
            desde: ahora - 5 * 86_400_000,
            conservadas: ["captura-0002.png".to_string()].into_iter().collect(),
            prorrogadas: Default::default(),
        };
        let mut e = Estado::nuevo(lista, registro);
        e.ocr = Some(true);
        e.leidos.insert(
            e.lista[0].ruta.clone(),
            (
                "Presupuesto Obra Miraflores".into(),
                logica::plegar("Presupuesto Obra Miraflores"),
            ),
        );
        e
    }

    fn ubicacion() -> Ubicacion {
        Ubicacion::Portable {
            raiz: std::env::temp_dir().join(format!("pixpin-galeria-ub-{}", std::process::id())),
        }
    }

    #[test]
    fn filtrar_y_buscar_por_el_texto_de_dentro() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista.len(), 4);
        assert_eq!(
            e.foco.as_deref(),
            Some(e.lista[0].ruta.as_path()),
            "sin foco, la primera"
        );
        e.filtro = Filtro::Conservadas;
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![1]);
        e.filtro = Filtro::Videos;
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![2]);
        e.filtro = Filtro::Todas;
        e.consulta = "miraflores".into();
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![0], "encuentra por el texto reconocido");
        e.consulta = "0001".into();
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![3], "y por el nombre");
        // Caso negativo: lo que no esta en ninguna, nada.
        e.consulta = "ladrillo".into();
        preparar(&mut e, 684.0, 1.0, &textos);
        assert!(e.vista.is_empty());
    }

    #[test]
    fn escribir_va_al_buscador_y_el_espacio_elige() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let z = zonas(1280.0, 820.0, 1.0);
        // El espacio, sin buscador, elige la del foco.
        caracter(&mut e, ' ');
        tecla(&mut e, 0x20, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(e.eligiendo && e.elegidas.contains(&e.lista[0].ruta));
        assert!(
            e.consulta.is_empty(),
            "caso negativo: el espacio no se escribe"
        );
        // Una letra enfoca el buscador y se escribe.
        caracter(&mut e, 'm');
        caracter(&mut e, 'i');
        assert!(e.escribiendo);
        assert_eq!(e.consulta, "mi");
        // Ahi el espacio si se escribe, y Retroceso borra.
        caracter(&mut e, ' ');
        tecla(&mut e, 0x08, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.consulta, "mi");
        // Esc limpia; otro Esc sale del buscador; otro sale de elegir; y el
        // ultimo cierra.
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(e.consulta.is_empty() && e.escribiendo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!e.escribiendo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!e.eligiendo && e.elegidas.is_empty() && vivo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!vivo);
        // Caso negativo: Ctrl+C no escribe una «c» (llega como control).
        caracter(&mut e, '\u{3}');
        assert!(e.consulta.is_empty());
    }

    #[test]
    fn las_flechas_mueven_el_foco_y_ctrl_a_elige_todas_las_que_se_ven() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let z = zonas(1280.0, 820.0, 1.0);
        tecla(&mut e, 0x27, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(1));
        // Abajo: de la fila de hoy al grupo de hace diez dias.
        tecla(&mut e, 0x28, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(3));
        e.filtro = Filtro::Hoy;
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_ne!(e.foco_i(), Some(3), "el foco se va de la que ya no se ve");
        tecla(
            &mut e,
            u32::from(b'A'),
            true,
            z,
            1.0,
            &textos,
            &ub,
            &mut vivo,
        );
        assert_eq!(e.elegidas.len(), 3);
        assert_eq!(e.objetivos(), vec![0, 1, 2]);
        // Otra vez Ctrl+A las suelta; sin elegidas, se actua sobre el foco.
        tecla(
            &mut e,
            u32::from(b'A'),
            true,
            z,
            1.0,
            &textos,
            &ub,
            &mut vivo,
        );
        assert!(e.elegidas.is_empty());
        assert_eq!(e.objetivos().len(), 1);
    }

    #[test]
    fn elegir_el_dia_elige_solo_los_de_ese_dia() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let viejo = e.disp.grupos[1].dia;
        hacer(
            &mut e,
            Accion::ElegirDia(viejo),
            false,
            &textos,
            &ub,
            &mut vivo,
        );
        assert!(e.eligiendo);
        assert_eq!(e.objetivos(), vec![3]);
        // Caso negativo: el mismo enlace otra vez las suelta.
        hacer(
            &mut e,
            Accion::ElegirDia(viejo),
            false,
            &textos,
            &ub,
            &mut vivo,
        );
        assert!(e.elegidas.is_empty());
    }

    #[test]
    fn el_doble_clic_es_la_misma_accion_y_un_clic_solo_enfoca() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        hacer(&mut e, Accion::Celda(2), false, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(2));
        assert!(!e.eligiendo, "caso negativo: un clic no entra a elegir");
        // Eligiendo, el clic marca y desmarca.
        hacer(
            &mut e,
            Accion::AlternarEligiendo,
            false,
            &textos,
            &ub,
            &mut vivo,
        );
        hacer(&mut e, Accion::Celda(1), false, &textos, &ub, &mut vivo);
        hacer(&mut e, Accion::Celda(3), false, &textos, &ub, &mut vivo);
        assert_eq!(e.objetivos(), vec![1, 3]);
        hacer(&mut e, Accion::Celda(1), false, &textos, &ub, &mut vivo);
        assert_eq!(e.objetivos(), vec![3]);
    }

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_galeria() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let mut e = estado();
        let ahora = e.ahora;
        // Mas capturas, para que salgan los tres grupos y los cuatro colores.
        let seg = |ms: i64| (ms / 1000) as u64;
        for k in 4..9 {
            e.lista.push(entrada(
                &format!("captura-00{k:02}.gif"),
                seg(ahora) - 86_400 - k * 60,
            ));
        }
        e.lista
            .push(entrada("captura-0020.png", seg(ahora) - 6 * 86_400));
        ordenar(&mut e.lista);
        e.registro.desde = ahora - 20 * 86_400_000;
        e.foco = Some(e.lista[0].ruta.clone());
        e.eligiendo = true;
        e.elegidas.insert(e.lista[0].ruta.clone());
        e.elegidas.insert(e.lista[2].ruta.clone());
        e.consulta = String::new();
        e.aviso = Some(Aviso {
            texto: con_cuantas(&textos, "galeria-borradas", 2),
            desde: Instant::now(),
            deshacer: true,
        });
        e.deshacer = vec![Borrada {
            original: PathBuf::from("x.png"),
            destino: PathBuf::from("y.png"),
            conservada: false,
            prorroga: None,
        }];
        preparar(
            &mut e,
            zonas(1280.0, 820.0, 1.0).rejilla.ancho,
            1.0,
            &textos,
        );
        // El raton sobre la segunda celda: salen sus botones.
        let c = e.disp.celdas[1];
        let z = zonas(1280.0, 820.0, 1.0);
        e.botones.raton = (z.rejilla.x + c.x + 30.0, z.rejilla.y + c.y + 60.0);
        crate::ventanita::muestra("galeria-capturas-v2", 1280, 820, |p, _| {
            pintar_todo(&mut e, p, 1280.0, 820.0, 1.0, &textos)
        });
        // Y sin elegir, con el buscador escribiendo.
        let mut e2 = estado();
        e2.escribiendo = true;
        e2.consulta = "obra".into();
        preparar(&mut e2, z.rejilla.ancho, 1.0, &textos);
        crate::ventanita::muestra("galeria-capturas-v2-buscar", 1280, 820, |p, _| {
            pintar_todo(&mut e2, p, 1280.0, 820.0, 1.0, &textos)
        });
    }
}
