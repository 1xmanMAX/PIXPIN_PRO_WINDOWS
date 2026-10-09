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
//! **La v2** (maqueta `Galeria2.dc.html`, 4-oct): buscador (que tambien
//! encuentra el texto de dentro de las capturas, leido con el OCR de
//! Windows en segundo plano), las capturas agrupadas por dia, la caducidad
//! por colores (gris, naranja, rojo y verde de conservada), un modo de
//! elegir varias con su barra de acciones, y un panel de detalle con
//! «Conservar» y «Dar 7 dias mas». Lo que se decide sin pintar vive en
//! [`logica`].
//!
//! El 5-oct el usuario pidio quitar elementos que sobraban: la barra
//! lateral de filtros (Hoy, Esta semana, Papelera... «no se usan»), la
//! vista previa pequena del panel y el texto reconocido a la vista (en el
//! panel y la chapita «Aa texto» de cada miniatura). El texto se sigue
//! leyendo en segundo plano: es lo que deja buscar por lo que pone dentro.
//!
//! El 6-oct pasa al sistema de diseno comun (`crate::v2`) con lo que pidio
//! el revisor: el foco solo tras un clic o una flecha (un solo anillo azul
//! de esquinas rectas), un «⋯» al pasar por una celda que abre el mismo
//! menu que el clic derecho, «hoy» y una pista en la chapita de los dias,
//! la barra de elegidas con los botones v2 (y «Guardar como») y el buscador
//! como una caja de verdad (`ui::Campo`).

#![forbid(unsafe_code)]

mod logica;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::lecciones::ui::{Campo, boton_v2};
use crate::overlay::Recursos;
use crate::v2::color::{blanco, negro, oscurecer};
use crate::v2::geom::{cortar, encoger};
use crate::v2::{self as d, aviso::Aviso};
use crate::ventanita::{Botones, centrado, dentro};

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
            let hecho = crate::dispositivo_perdido::con_recursos("galeria", |r| {
                bucle(r, &textos, &ubicacion)
            });
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

/// **La ruta de una captura nueva**: `captura-AAAAMMDD-HHMMSS.png` con la
/// hora local `local_ms` (ms desde 1970 ya corridos al huso), y `-2`, `-3`…
/// si en ese segundo ya hay otra con ese nombre (con cualquiera de las
/// extensiones de la galeria: un GIF o un MP4 sale de la misma ruta).
///
/// Antes era `captura-NNNN` con el primer numero libre, y el numero se
/// reutilizaba al borrar. Con la galeria que viaja (9-oct) eso no vale: el
/// nombre es la clave de cada captura en todos los aparatos, y una nueva
/// con el nombre de una borrada heredaria su marca de borrada y se iria de
/// todos. Las `captura-NNNN` que ya habia se quedan como estan: como ya no
/// se reparten numeros, su nombre no lo vuelve a coger ninguna.
pub fn ruta_nueva_en(carpeta: &Path, local_ms: i64) -> PathBuf {
    let base = nombre_por_hora(local_ms);
    let ocupado = |stem: &str| {
        EXTENSIONES
            .iter()
            .any(|e| carpeta.join(format!("{stem}.{e}")).exists())
    };
    let mut stem = base.clone();
    let mut n = 2;
    while ocupado(&stem) {
        stem = format!("{base}-{n}");
        n += 1;
    }
    carpeta.join(format!("{stem}.png"))
}

/// `captura-AAAAMMDD-HHMMSS` de una hora local en ms desde 1970.
fn nombre_por_hora(local_ms: i64) -> String {
    let s = local_ms.div_euclid(1000);
    let (dias, resto) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // Dias desde 1970 a fecha civil (Howard Hinnant, `civil_from_days`).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "captura-{a:04}{m:02}{d:02}-{:02}{:02}{:02}",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
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

/// Lo que devuelve el hilo de las miniaturas: la captura y su miniatura
/// (`None` si no se pudo leer).
type Hecha = (PathBuf, Option<ImagenRgba>);

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
                if dar.send((entrada.ruta, mini)).is_err() {
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
/// Windows (`pixpin_ocr`), para buscar por lo que pone dentro (no se
/// ensena: el usuario lo quito de la vista el 5-oct). Una captura de pantalla tarda de 170 a 670 ms (medido
/// en `main.rs`): por eso va en su hilo, de una en una, con un respiro entre
/// una y otra para no quitarle el procesador al equipo, y lo leido se
/// guarda en la cache: la segunda vez que se abre la galeria no se relee.
///
/// Como las miniaturas, atiende primero lo ULTIMO pedido: la ventana pide
/// todas al abrir (de la mas vieja a la mas nueva), asi que las nuevas,
/// que son las que mas se buscan, se leen antes.
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

// Los colores, radios y medidas son los del sistema v2 (`crate::v2`): la
// galeria es oscura como las demas ventanas del rediseno.
const TECLA_DOBLE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Accion {
    /// Una zona que se traga el clic (el fondo de un panel, el aviso).
    #[default]
    Nada,
    Cerrar,
    Carpeta,
    Mover,
    Buscar,
    LimpiarBusqueda,
    AlternarEligiendo,
    SalirEligiendo,
    ElegirDia(i64),
    ElegirTodas,
    /// Las de abajo llevan la posicion en `lista`.
    Celda(usize),
    Marca(usize),
    VerGrande(usize),
    /// El «⋯» de una celda: el mismo menu que el clic derecho.
    Mas(usize),
    /// La barra de las elegidas.
    BarraConservar,
    BarraPinear,
    BarraCopiar,
    BarraGuardar,
    BarraBorrar,
    Deshacer,
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
    /// El texto reconocido de cada una, ya plegado: solo sirve para buscar
    /// (no se ensena), asi que no hace falta guardarlo tal cual.
    leidos: HashMap<PathBuf, String>,
    /// Si hay OCR: `None` mientras no se sabe.
    ocr: Option<bool>,
    /// El buscador: una caja de verdad (cursor, flechas, Ctrl+Retroceso,
    /// Ctrl+V), la misma de las lecciones.
    consulta: Campo,
    /// El cursor esta en el buscador: las letras van ahi.
    escribiendo: bool,
    /// Las posiciones de `lista` que se ven, en orden.
    vista: Vec<usize>,
    disp: logica::Disposicion,
    /// La captura con el foco del teclado. Nace vacio y solo se pone con un
    /// clic o una flecha: con un foco puesto solo, Intro o Supr nada mas
    /// abrir actuaban sobre una captura que nadie habia elegido.
    foco: Option<PathBuf>,
    eligiendo: bool,
    elegidas: BTreeSet<PathBuf>,
    scroll: f32,
    botones: Botones<Accion>,
    aviso: Option<Aviso<Accion>>,
    /// Lo ultimo borrado, para «Deshacer».
    deshacer: Vec<Borrada>,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    ultimo_clic: Option<(Accion, Instant)>,
    /// El desplazamiento con el que se pinto lo que se ve: las zonas de los
    /// botones son de entonces.
    scroll_pintado: f32,
    /// Un clic que llego con la rejilla movida desde el ultimo pintado (la
    /// rueda y el clic en la misma tanda): se atiende tras repintar, para que
    /// caiga en la foto que se ve y no en la de antes.
    clic_pendiente: Option<pixpin_geom::Punto>,
    /// Lo mismo para el clic derecho: con la rueda de por medio, la celda
    /// bajo el raton es la de despues de repintar, no la de antes.
    derecho_pendiente: Option<pixpin_geom::Punto>,
    /// El menu de la captura `.0`: sale tras repintar, con el foco ya en
    /// ella.
    menu_pendiente: Option<usize>,
    /// «Guardar como» de la barra: el dialogo necesita la ventana, que
    /// `hacer` no tiene; sale tras repintar, como el menu.
    guardar_pendiente: bool,
    fotograma: u64,
    ahora: i64,
}

impl Estado {
    fn nuevo(lista: Vec<Entrada>, registro: crate::caducidad_capturas::Registro) -> Self {
        Estado {
            lista,
            registro,
            minis: HashMap::new(),
            leidos: HashMap::new(),
            ocr: None,
            consulta: Campo::default(),
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
            scroll_pintado: 0.0,
            clic_pendiente: None,
            derecho_pendiente: None,
            menu_pendiente: None,
            guardar_pendiente: false,
            fotograma: 0,
            ahora: pixpin_shell::entorno::ahora_utc_ms(),
        }
    }

    fn se_va(&self, i: usize) -> Option<i64> {
        crate::caducidad_capturas::se_va_el(&self.registro, &self.lista[i])
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
        self.aviso = Some(Aviso::nuevo(texto));
    }

    /// Vacia el buscador y vuelve arriba.
    fn vaciar_busqueda(&mut self) {
        self.consulta = Campo::default();
        self.scroll = 0.0;
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
        .map(String::as_str)
        .unwrap_or("");
    logica::encaja(consulta, &[&nombre, &fecha, dentro])
}

/// Rehace la vista (la busqueda) y su disposicion para un ancho de
/// rejilla. Barato: se hace antes de cada fotograma y de cada tecla. Sin
/// filtros: se quitaron con el carril (5-oct), asi que se ven todas.
fn preparar(e: &mut Estado, ancho_rejilla: f32, escala: f32, textos: &Catalogo) {
    e.ahora = pixpin_shell::entorno::ahora_utc_ms();
    let consulta = logica::plegar(e.consulta.texto.trim());
    let vista: Vec<usize> = (0..e.lista.len())
        .filter(|&i| coincide(e, i, &consulta, textos))
        .collect();
    let dias: Vec<i64> = vista
        .iter()
        .map(|&i| logica::dia_local(crate::caducidad_capturas::ms_de(e.lista[i].cuando)))
        .collect();
    e.disp = logica::disponer(&dias, ancho_rejilla, escala);
    e.vista = vista;
    // Un foco que la busqueda esconde (o que se borro) se suelta: Intro o
    // Supr no deben actuar sobre una captura que no se ve. Y no se pone
    // ninguno solo: ver `Estado::foco`.
    if e.foco.is_some() && e.foco_en_vista().is_none() {
        e.foco = None;
    }
    // Las elegidas que se fueron de la carpeta dejan de estarlo.
    if !e.elegidas.is_empty() {
        let presentes: std::collections::HashSet<&PathBuf> =
            e.lista.iter().map(|x| &x.ruta).collect();
        e.elegidas.retain(|r| presentes.contains(r));
    }
}

/// Las zonas de la ventana, en pixeles de la ventana. Sin carril a la
/// izquierda ni panel de detalle a la derecha (5-oct): la rejilla es toda la
/// ventana bajo la cabecera, y lo que se hace con una captura sale con el
/// clic derecho.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Zonas {
    rejilla: RectF,
}

fn zonas(w: f32, h: f32, escala: f32) -> Zonas {
    let cab = logica::CABECERA * escala;
    Zonas {
        rejilla: RectF {
            x: 0.0,
            y: cab,
            ancho: w.max(0.0),
            alto: (h - cab).max(0.0),
        },
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
                    if (e.scroll - e.scroll_pintado).abs() > 0.5 {
                        // Las zonas son de antes de la rueda: tras repintar.
                        e.clic_pendiente = Some(p);
                    } else {
                        clic(
                            &mut e, p, marco, &ventana, z, escala, textos, ubicacion, &mut vivo,
                        );
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => {
                    // `m` viene en 120 por muesca (y en trozos pequenos
                    // desde un panel tactil): 0,6 filas por muesca. Sin
                    // dividir, cada muesca saltaba al principio o al final.
                    e.scroll = (e.scroll
                        - m as f32 / 120.0 * (logica::CELDA_ALTO + logica::HUECO) * escala * 0.6)
                        .clamp(0.0, e.disp.scroll_maximo(z.rejilla.alto));
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    if (e.scroll - e.scroll_pintado).abs() > 0.5 {
                        // Como el izquierdo: las zonas son de antes de la
                        // rueda, y el menu saldria de otra captura.
                        e.derecho_pendiente = Some(p);
                    } else {
                        clic_derecho(&mut e);
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
        while let Ok((ruta, mini)) = hechas.try_recv() {
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
                Leido::SinOcr => {
                    // Cambia la pista del buscador.
                    e.ocr = Some(false);
                    pintar = true;
                }
                Leido::Texto(ruta, t) => {
                    e.ocr = Some(true);
                    e.leidos.insert(ruta, logica::plegar(&t));
                    // El texto ya no se ensena: solo cambia lo que se ve si
                    // hay algo buscado (puede aparecer una que lo contiene).
                    // Sin eso, no se repinta por cada una de cientos.
                    if !e.consulta.texto.trim().is_empty() {
                        pintar = true;
                    }
                }
            }
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
        if e.aviso.as_ref().is_some_and(Aviso::caducado) {
            e.aviso = None;
            pintar = true;
        }

        if pintar {
            e.fotograma += 1;
            preparar(&mut e, z.rejilla.ancho, escala, textos);
            e.scroll = e.scroll.clamp(0.0, e.disp.scroll_maximo(z.rejilla.alto));
            pedir_y_subir(&mut e, z, &pedir, &cache, &motor);
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, w, h, escala, textos)
                });
                let _ = superficie.presentar();
            }
            e.scroll_pintado = e.scroll;
            soltar_lejanas(&mut e, z);
            pintar = false;
            if let Some(p) = e.clic_pendiente.take() {
                e.botones.raton = local_de(p, marco);
                clic(
                    &mut e, p, marco, &ventana, z, escala, textos, ubicacion, &mut vivo,
                );
                // Lo que cambio el clic se ve ya, sin esperar a otro evento.
                pintar = true;
                continue;
            }
            if let Some(p) = e.derecho_pendiente.take() {
                e.botones.raton = local_de(p, marco);
                clic_derecho(&mut e);
                pintar = true;
                continue;
            }
            if let Some(i) = e.menu_pendiente.take() {
                menu_de_captura(&mut e, i, &ventana, textos, ubicacion);
                pintar = true;
                continue;
            }
            if std::mem::take(&mut e.guardar_pendiente) {
                let v = e.objetivos();
                guardar_como(&mut e, &v, &ventana, textos);
                pintar = true;
                continue;
            }
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

/// Un punto de pantalla en coordenadas de la ventana.
fn local_de(p: pixpin_geom::Punto, m: Rect) -> (f32, f32) {
    ((p.x - m.x) as f32, (p.y - m.y) as f32)
}

/// Un clic derecho, sobre las zonas del ultimo pintado: sobre una captura
/// (su celda, su circulo o su «⋯»), su menu. El menu sale tras repintar:
/// asi se ve antes cual es la captura de la que se habla (el foco salta a
/// ella). Devuelve si habia captura.
fn clic_derecho(e: &mut Estado) -> bool {
    let Some(Accion::Celda(i) | Accion::Marca(i) | Accion::Mas(i)) = e.botones.bajo_el_raton()
    else {
        return false;
    };
    let Some(r) = e.lista.get(i).map(|x| x.ruta.clone()) else {
        return false;
    };
    e.escribiendo = false;
    e.foco = Some(r);
    e.menu_pendiente = Some(i);
    true
}

/// Un clic con el boton izquierdo, sobre las zonas del ultimo pintado.
#[allow(clippy::too_many_arguments)] // el bucle de la ventana, entero
fn clic(
    e: &mut Estado,
    p: pixpin_geom::Punto,
    marco: Rect,
    ventana: &VentanaOverlay,
    z: Zonas,
    escala: f32,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    vivo: &mut bool,
) {
    match e.botones.bajo_el_raton() {
        Some(Accion::Mover) => {
            e.escribiendo = false;
            e.moviendo = Some((p, marco));
            ventana.capturar_raton();
        }
        Some(a) => {
            preparar(e, z.rejilla.ancho, escala, textos);
            let doble = e
                .ultimo_clic
                .is_some_and(|(b, t)| b == a && t.elapsed() < TECLA_DOBLE);
            e.ultimo_clic = Some((a, Instant::now()));
            hacer(e, a, doble, textos, ubicacion, vivo);
        }
        None => e.escribiendo = false,
    }
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

fn fecha_de(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// Pide al hilo las miniaturas a la vista que faltan y sube a la GPU las
/// que ya llegaron. Se hace ANTES de pintar: crear bitmaps a medio dibujo
/// no se puede.
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
    // Al reves: el hilo atiende primero lo ultimo pedido, y asi lo de arriba
    // de la vista llega antes.
    let quiero: Vec<usize> = e.vista[rango].iter().rev().copied().collect();
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
    let a_la_vista: std::collections::HashSet<&PathBuf> =
        e.vista[rango].iter().map(|&i| &e.lista[i].ruta).collect();
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
            ESC if !e.consulta.texto.is_empty() => e.vaciar_busqueda(),
            ESC | INTRO | 0x28 => e.escribiendo = false,
            _ => {
                // Lo demas es de la caja (Retroceso, Ctrl+Retroceso, Supr,
                // las flechas, Inicio/Fin, Ctrl+V). Si cambio lo buscado, la
                // rejilla vuelve arriba: lo de antes ya no es lo mismo.
                let antes = e.consulta.texto.clone();
                e.consulta.tecla(vk, ctrl, false, false);
                if e.consulta.texto != antes {
                    e.scroll = 0.0;
                }
            }
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
        ESC if !e.consulta.texto.is_empty() => e.vaciar_busqueda(),
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
    if e.consulta.letra(c) {
        e.scroll = 0.0;
    }
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
        Accion::Carpeta => {
            let dir = carpeta(ubicacion);
            let _ = std::fs::create_dir_all(&dir);
            if let Err(err) = pixpin_shell::abrir(&dir) {
                e.avisar(fallo(textos, err.to_string()));
            }
        }
        Accion::Buscar => e.escribiendo = true,
        Accion::LimpiarBusqueda => {
            e.vaciar_busqueda();
            e.escribiendo = true;
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
        Accion::BarraGuardar => e.guardar_pendiente = !e.objetivos().is_empty(),
        Accion::Mas(i) => {
            if let Some(r) = e.lista.get(i).map(|x| x.ruta.clone()) {
                e.foco = Some(r);
                e.menu_pendiente = Some(i);
            }
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
            if let Some(b) = borradas.first() {
                e.foco = Some(b.original.clone());
            }
            e.avisar(con_cuantas(textos, "galeria-recuperadas", vueltas));
        }
    }
}

/// **El menu del clic derecho** (5-oct): lo que antes eran los botones de
/// encima de cada miniatura y el panel de detalle de la derecha. El usuario:
/// «que los botones de pinear, copiar y borrar aparezcan solo cuando le doy
/// clic derecho, y que aparezca abrir carpeta y guardar como también; la
/// barra lateral quítala porque el usuario se puede confundir». Conservar
/// tambien va aqui: sin el panel no quedaba otro sitio, y sin el las
/// capturas se irian a los siete dias sin remedio.
///
/// Sobre una de las elegidas, el menu vale para todas ellas.
fn menu_de_captura(
    e: &mut Estado,
    i: usize,
    ventana: &VentanaOverlay,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
) {
    let raiz = ubicacion.raiz();
    let sobre_elegida = e.eligiendo
        && e
            .lista
            .get(i)
            .is_some_and(|x| e.elegidas.contains(&x.ruta));
    let v: Vec<usize> = if sobre_elegida {
        e.objetivos()
    } else {
        vec![i]
    };
    if v.is_empty() {
        return;
    }
    let alguna_caduca = v.iter().any(|&k| e.se_va(k).is_some());
    let mut entradas = vec![
        (1, format!("{}\tEnter", textos.t("galeria-pinear"))),
        (2, format!("{}\tCtrl+C", textos.t("galeria-copiar"))),
        (8, textos.t("salida-compartir-menu")),
        (3, textos.t("galeria-guardar-como")),
        (4, textos.t("galeria-mostrar-en-carpeta")),
    ];
    if alguna_caduca {
        entradas.push((5, format!("{}\tCtrl+S", textos.t("galeria-conservar"))));
        entradas.push((7, textos.t("galeria-prorrogar")));
    }
    entradas.push((6, format!("{}\tSupr", textos.t("galeria-borrar"))));
    match pixpin_shell::menu_llano(ventana.handle(), &entradas) {
        Some(1) => pinear(e, &v, textos),
        Some(2) => copiar(e, &v, textos),
        // A la Salida: las capturas ya son planas (lo anotado va dentro), y
        // desde ahi se arrastran a un chat o se mandan con el panel.
        Some(8) => {
            let rutas: Vec<PathBuf> = v
                .iter()
                .filter_map(|&k| e.lista.get(k).map(|x| x.ruta.clone()))
                .collect();
            crate::salida::mostrar(rutas, textos.t("salida-titulo"));
        }
        Some(3) => guardar_como(e, &v, ventana, textos),
        Some(4) => {
            if let Some(x) = e.lista.get(v[0]) {
                mostrar_en_carpeta(&x.ruta);
            }
        }
        Some(5) => conservar_varias(e, &v, raiz, textos),
        Some(7) => prorrogar_varias(e, &v, raiz, textos),
        Some(6) => borrar_varias(e, &v, raiz, textos),
        _ => {}
    }
}

/// «Dar 7 dias mas» a las que aun se van (lo que hacia el panel de detalle).
fn prorrogar_varias(e: &mut Estado, v: &[usize], raiz: &Path, textos: &Catalogo) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    for &k in v {
        if e.se_va(k).is_none() {
            continue;
        }
        let Some(x) = e.lista.get(k).cloned() else {
            continue;
        };
        match crate::caducidad_capturas::prorrogar(raiz, &x, ahora) {
            Ok(reg) => e.registro = reg,
            Err(err) => {
                e.avisar(fallo(textos, err.to_string()));
                return;
            }
        }
    }
    e.avisar(textos.t("galeria-prorrogada"));
}

/// La carpeta de la captura, con ella ya senalada (`explorer /select,`).
fn mostrar_en_carpeta(ruta: &Path) {
    use std::os::windows::process::CommandExt;
    // `raw_arg`: Explorer quiere las comillas solo alrededor de la ruta; con
    // las que pone `arg` alrededor de todo, abre «Documentos» y no senala nada.
    let lanzado = std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", ruta.display()))
        .spawn();
    if let Err(err) = lanzado {
        tracing::warn!(?err, "galeria: no se pudo abrir la carpeta");
    }
}

/// «Guardar como…»: una captura, con el dialogo de guardar de Windows;
/// varias, eligiendo la carpeta (sin pisar lo que ya hubiera alli).
fn guardar_como(e: &mut Estado, v: &[usize], ventana: &VentanaOverlay, textos: &Catalogo) {
    let rutas: Vec<PathBuf> = v
        .iter()
        .filter_map(|&k| e.lista.get(k).map(|x| x.ruta.clone()))
        .collect();
    let hwnd = ventana.handle();
    let copiadas = if let [una] = rutas.as_slice() {
        let nombre = una
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = una
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let tipo = ext.to_ascii_uppercase();
        let Some(destino) = pixpin_shell::guardar::pedir_ruta_para(hwnd, &nombre, &tipo, &ext) else {
            return;
        };
        match std::fs::copy(una, &destino) {
            Ok(_) => 1,
            Err(err) => {
                e.avisar(fallo(textos, err.to_string()));
                return;
            }
        }
    } else {
        let Some(dir) = pixpin_shell::guardar::pedir_carpeta(hwnd) else {
            return;
        };
        let mut n = 0;
        for r in &rutas {
            let Some(nombre) = r.file_name() else { continue };
            let mut destino = dir.join(nombre);
            let mut k = 2;
            while destino.exists() {
                let base = r.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ext = r.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                destino = dir.join(format!("{base} ({k}).{ext}"));
                k += 1;
            }
            if std::fs::copy(r, &destino).is_ok() {
                n += 1;
            }
        }
        n
    };
    e.avisar(con_cuantas(textos, "galeria-guardadas", copiadas));
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
        // Sin nada que conservar, pero con algo pedido: que se sepa por que
        // no paso nada (Ctrl+S sobre una ya conservada).
        if !v.is_empty() {
            e.avisar(textos.t("galeria-ya-conservadas"));
        }
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
    if let Some(m) = error {
        e.avisar(fallo(textos, m));
        e.deshacer = borradas;
        return;
    }
    let n = borradas.len();
    e.deshacer = borradas;
    let texto = if n == 1 {
        textos.t("galeria-borrada")
    } else {
        con_cuantas(textos, "galeria-borradas", n)
    };
    e.aviso = Some(Aviso::con_deshacer(
        texto,
        textos.t("galeria-deshacer"),
        Accion::Deshacer,
    ));
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

/// El fondo de los botones de la barra de elegidas: un punto mas claro que
/// la barra (CAJA), para que se vean como botones y no como texto suelto.
const BOTON_BARRA: Color = crate::caja_dibujo::hex(0x3B3B3E);

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

fn pintar_todo(e: &mut Estado, p: &Pintor, w: f32, h: f32, escala: f32, textos: &Catalogo) {
    p.limpiar(d::FONDO);
    e.botones.vaciar();
    let z = zonas(w, h, escala);
    pintar_rejilla(e, p, z, escala, textos);
    pintar_cabecera(e, p, w, escala, textos);
}

/// La barra de arriba, la comun de las cuatro ventanas (`crate::cabecera`):
/// el titulo, el buscador y, como botones propios, elegir varias y abrir la
/// carpeta.
fn pintar_cabecera(e: &mut Estado, p: &Pintor, w: f32, escala: f32, textos: &Catalogo) {
    let titulo = textos.t("galeria-titulo");
    let subtitulo = con_cuantas(textos, "galeria-subtitulo", e.lista.len());
    let pista = if e.ocr == Some(false) {
        textos.t("galeria-buscar-sin-ocr")
    } else {
        textos.t("galeria-buscar")
    };
    // El mismo rotulo encendido o apagado: si cambiara («Seleccionando»),
    // el boton cambiaria de ancho y empujaria al buscador al pulsarlo. El
    // estado lo dice el azul.
    let elegir = textos.t("galeria-elegir");
    let carpeta = textos.t("galeria-abrir-carpeta");
    let c = crate::cabecera::Cabecera {
        titulo: &titulo,
        subtitulo: &subtitulo,
        buscador: Some(crate::cabecera::Buscador {
            texto: &e.consulta.texto,
            pista: &pista,
            foco: e.escribiendo,
            cursor: Some(e.consulta.cursor),
            enfocar: Accion::Buscar,
            vaciar: Accion::LimpiarBusqueda,
        }),
        botones: vec![
            crate::cabecera::Boton {
                icono: Some(&mi::CHECK_BOX),
                rotulo: &elegir,
                chapa: None,
                activo: e.eligiendo,
                accion: Accion::AlternarEligiendo,
            },
            crate::cabecera::Boton {
                icono: Some(&mi::FOLDER),
                rotulo: &carpeta,
                chapa: None,
                activo: false,
                accion: Accion::Carpeta,
            },
        ],
        mover: Accion::Mover,
        cerrar: Accion::Cerrar,
    };
    crate::cabecera::pintar(p, &mut e.botones, w, escala, &c);
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

/// Lo que dice la chapita de los dias de una captura: el numero, y «hoy»
/// el ultimo dia (un 0 rojo parecia un error).
fn rotulo_de_dias(textos: &Catalogo, dias: i64) -> String {
    if dias <= 0 {
        textos.t("galeria-chapa-hoy")
    } else {
        dias.to_string()
    }
}

/// La pista al pasar por la chapita: cuando se va y como evitarlo.
fn pista_de_dias(textos: &Catalogo, dias: i64) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("dias", dias.max(0));
    textos.t_args("galeria-pista-caduca", &args)
}

fn pintar_rejilla(e: &mut Estado, p: &Pintor, z: Zonas, escala: f32, textos: &Catalogo) {
    let r = z.rejilla;
    if e.vista.is_empty() {
        // Sin filtros, con capturas y nada a la vista solo puede ser la
        // busqueda.
        let (icono, t) = if e.lista.is_empty() {
            (&mi::PHOTO_LIBRARY, textos.t("galeria-vacia"))
        } else {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("texto", e.consulta.texto.trim().to_string());
            (&mi::SEARCH, textos.t_args("galeria-sin-resultados", &args))
        };
        d::vacio(p, r, icono, &t, escala);
    }
    let encima = e.botones.raton;
    let hoy = logica::dia_local(e.ahora);
    let (ox, oy) = (r.x, r.y - e.scroll);
    let grupos = e.disp.grupos.clone();
    let rango = e.disp.visibles(e.scroll - 4.0, e.scroll + r.alto + 4.0);
    let mut pista = None;
    p.con_recorte(r, |p| {
        // Los titulos de los dias que asoman.
        for g in &grupos {
            let y = oy + g.y;
            if y + logica::TITULO_DIA * escala < r.y || y > r.y + r.alto {
                continue;
            }
            let (titulo, sub) = titulo_del_dia(textos, g.dia, hoy);
            let x = ox + logica::LADO_REJILLA * escala;
            let tam = d::LETRA_CUERPO * escala;
            let (tw, th) = crate::lecciones::ui::medir_negrita(p, &titulo, tam, 10_000.0);
            let yt = y + (logica::TITULO_DIA * escala - th) / 2.0;
            crate::lecciones::ui::negrita(p, &titulo, x, yt, tam, 10_000.0, d::TEXTO);
            let sub = format!(
                "{sub} · {}",
                con_cuantas(textos, "galeria-dia-cuantas", g.hasta - g.desde)
            );
            p.texto(
                &sub,
                x + tw + 10.0 * escala,
                yt + 1.0 * escala,
                d::LETRA_SECUNDARIO * escala,
                d::GRIS,
            );
            // «Elegir todo el dia», solo eligiendo: fuera de ese modo era un
            // enlace azul en cada grupo que nadie buscaba (el revisor).
            if !e.eligiendo {
                continue;
            }
            let enlace = textos.t("galeria-elegir-dia");
            let tam_e = d::LETRA_SECUNDARIO * escala;
            let (ew, eh) = p.medir_texto(&enlace, tam_e);
            let ze = RectF {
                x: r.x + r.ancho - logica::LADO_REJILLA * escala - ew - 8.0 * escala,
                y: y + (logica::TITULO_DIA * escala - d::OBJETIVO_MINIMO * escala) / 2.0,
                ancho: ew + 8.0 * escala,
                alto: d::OBJETIVO_MINIMO * escala,
            };
            let color = if dentro(ze, encima) {
                d::color::aclarar(d::ACENTO, 0.15)
            } else {
                d::ACENTO
            };
            p.texto(
                &enlace,
                ze.x + 4.0 * escala,
                ze.y + (ze.alto - eh) / 2.0,
                tam_e,
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
            if let Some(pi) = pintar_celda(e, p, c, r, i, escala, textos) {
                pista = Some(pi);
            }
        }
    });
    // La pista, encima de todas las celdas (la de al lado no la tapa).
    if let Some((ancla, texto)) = pista {
        d::pista::pintar(p, ancla, &texto, r, escala);
    }
    let mut zona_aviso = r;
    if e.eligiendo {
        let barra = pintar_barra_elegidas(e, p, r, escala, textos);
        // El aviso, encima de la barra y no tapandola.
        zona_aviso.alto = (barra.y - r.y).max(0.0);
    }
    if let Some(a) = &e.aviso {
        d::aviso::pintar(a, p, &mut e.botones, zona_aviso, escala);
    }
}

/// Pinta una celda y apunta sus zonas. Devuelve la pista que hay que
/// ensenar si el raton esta sobre su chapita (se pinta despues de todas).
#[allow(clippy::too_many_arguments)] // estado, pintor, celda, zona, indice, escala, textos
fn pintar_celda(
    e: &mut Estado,
    p: &Pintor,
    c: RectF,
    zona: RectF,
    i: usize,
    escala: f32,
    textos: &Catalogo,
) -> Option<(RectF, String)> {
    let entrada = e.lista[i].clone();
    let raton = e.botones.raton;
    let encima = dentro(c, raton) && dentro(zona, raton);
    let elegida = e.elegidas.contains(&entrada.ruta);
    let enfocada = e.foco.as_ref() == Some(&entrada.ruta);
    // El marco del foco y de elegida: UN anillo azul de 2 px, a 3 de la
    // celda, con esquinas RECTAS como la captura (5-oct: «aparece un marco
    // con esquina redondeada pero la captura tiene esquina rectangular»).
    // El blanco de fuera que llevaba el foco se quito: dos anillos se leian
    // como dos estados distintos.
    if enfocada || elegida {
        p.rellenar(encoger(c, -3.0 * escala), d::ACENTO);
        p.rellenar(encoger(c, -1.0 * escala), d::FONDO);
    } else if encima {
        p.rellenar(encoger(c, -1.0 * escala), blanco(0.35));
    }
    p.rellenar(c, d::TARJETA);
    match e.minis.get(&entrada.ruta) {
        Some(Mini::Lista {
            bitmap: Some(b),
            imagen,
            ..
        }) => crate::miniaturas::pintar_recortado(p, b, c, imagen.ancho, imagen.alto),
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
                d::GRIS,
            );
        }
    }
    if let Some(zc) = cortar(c, zona) {
        e.botones.zona(zc, Accion::Celda(i));
    }
    let mut pista = None;

    // Arriba a la derecha, la caducidad: solo los dias que le quedan, en
    // una chapita roja y pequena (5-oct: «que directamente solo muestre un
    // numero en la esquina con fondo rojo»). Las conservadas, una marca
    // verde. Lo que significa, en la pista al pasar por encima.
    let tam = 12.0 * escala;
    let lado = 22.0 * escala;
    match logica::dias_que_quedan(e.se_va(i), e.ahora) {
        Some(n) => {
            let t = rotulo_de_dias(textos, n);
            let (tw, th) = p.medir_texto(&t, tam);
            let ancho = (tw + 12.0 * escala).max(lado);
            let bd = RectF {
                x: c.x + c.ancho - 6.0 * escala - ancho,
                y: c.y + 6.0 * escala,
                ancho,
                alto: lado,
            };
            p.rellenar_redondeado(bd, lado / 2.0, d::ROJO);
            p.texto(
                &t,
                bd.x + (bd.ancho - tw) / 2.0,
                bd.y + (lado - th) / 2.0,
                tam,
                Color::BLANCO,
            );
            if encima && dentro(encoger(bd, -4.0 * escala), raton) {
                pista = Some((bd, pista_de_dias(textos, n)));
            }
        }
        None => {
            let centro = (
                c.x + c.ancho - 6.0 * escala - lado / 2.0,
                c.y + 6.0 * escala + lado / 2.0,
            );
            p.circulo(centro, lado / 2.0, oscurecer(d::VERDE, 0.12));
            pintar_check(p, centro, 12.0 * escala, Color::BLANCO, 2.0 * escala);
            let bd = d::geom::centrado(centro, lado);
            if encima && dentro(encoger(bd, -4.0 * escala), raton) {
                pista = Some((bd, textos.t("galeria-conservada-aviso")));
            }
        }
    }

    // Abajo a la izquierda, lo que es: GIF o video. Sin la de «Aa texto»
    // (5-oct): el usuario no quiere el texto reconocido a la vista.
    let ext = entrada
        .ruta
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let chip = match ext.as_str() {
        "mp4" => Some(textos.t("galeria-chip-video")),
        "gif" => Some("GIF".to_string()),
        _ => None,
    };
    if let Some(chip) = chip {
        let (cw, ch) = p.medir_texto(&chip, tam);
        let cc = RectF {
            x: c.x + 8.0 * escala,
            y: c.y + c.alto - 8.0 * escala - ch - 6.0 * escala,
            ancho: cw + 16.0 * escala,
            alto: ch + 6.0 * escala,
        };
        p.rellenar_redondeado(cc, 7.0 * escala, negro(0.75));
        p.texto(&chip, cc.x + 8.0 * escala, cc.y + 3.0 * escala, tam, d::CUERPO);
    }

    // Abajo a la derecha, al pasar, el «⋯»: la pista de que hay menu. El
    // clic derecho no lo adivina nadie (el revisor); el «⋯» abre el mismo.
    // Se ve de 28, pero su zona es de 40 (objetivo minimo).
    if encima {
        let zm = RectF {
            x: c.x + c.ancho - d::OBJETIVO_MINIMO * escala,
            y: c.y + c.alto - d::OBJETIVO_MINIMO * escala,
            ancho: d::OBJETIVO_MINIMO * escala,
            alto: d::OBJETIVO_MINIMO * escala,
        };
        let centro = (zm.x + zm.ancho / 2.0, zm.y + zm.alto / 2.0);
        let visto = d::geom::centrado(centro, 28.0 * escala);
        let sobre = dentro(zm, raton);
        p.rellenar_redondeado(visto, 8.0 * escala, negro(if sobre { 0.75 } else { 0.55 }));
        for k in [-1.0f32, 0.0, 1.0] {
            p.circulo(
                (centro.0 + k * 6.0 * escala, centro.1),
                1.8 * escala,
                Color::BLANCO,
            );
        }
        if let Some(z) = cortar(zm, zona) {
            e.botones.zona(z, Accion::Mas(i));
        }
    }

    // Arriba a la izquierda, la marca de elegir (al elegir, o al pasar). Se
    // ve de 26, pero se pulsa en 40.
    if e.eligiendo || encima {
        let dm = 26.0 * escala;
        let centro = (c.x + 8.0 * escala + dm / 2.0, c.y + 8.0 * escala + dm / 2.0);
        let zm = d::geom::centrado(centro, d::OBJETIVO_MINIMO * escala);
        if elegida {
            p.circulo(centro, dm / 2.0, d::ACENTO);
            pintar_check(p, centro, 16.0 * escala, Color::BLANCO, 2.5 * escala);
        } else {
            // Un canto oscuro por fuera: sobre una captura clara, el aro
            // blanco solo no se veia (quedaba una mancha gris).
            p.circulo(centro, dm / 2.0 + 1.0 * escala, negro(0.35));
            p.circulo(centro, dm / 2.0, negro(0.3));
            let borde = if dentro(zm, raton) {
                Color::BLANCO
            } else {
                blanco(0.85)
            };
            p.anillo(centro, dm / 2.0 - 1.0 * escala, 2.0 * escala, borde);
        }
        if let Some(zc) = cortar(zm, zona) {
            e.botones.zona(zc, Accion::Marca(i));
        }
    }
    pista
}

/// Un boton de la barra de elegidas: que hace, su icono, rotulo y atajo,
/// fondo (`None` = sin fondo, el de «Borrar») y color de la letra.
struct BotonBarra {
    accion: Accion,
    icono: &'static pixpin_render::icono::Icono,
    rotulo: String,
    chapa: &'static str,
    fondo: Option<Color>,
    tinta: Color,
}

/// La barra de abajo al elegir varias. De izquierda a derecha: cuantas,
/// «Todas»; y a la derecha Pinear (la principal, en azul y siempre en el
/// mismo sitio), Conservar, Copiar, Guardar como, una raya, Borrar
/// (apartado, en rojo) y salir. Devuelve la barra.
fn pintar_barra_elegidas(
    e: &mut Estado,
    p: &Pintor,
    r: RectF,
    escala: f32,
    textos: &Catalogo,
) -> RectF {
    let s = escala;
    let alto = 60.0 * s;
    let barra = RectF {
        x: r.x + 20.0 * s,
        y: r.y + r.alto - d::MARGEN * s - alto,
        ancho: r.ancho - 40.0 * s,
        alto,
    };
    p.rellenar_redondeado(encoger(barra, -8.0 * s), 20.0 * s, negro(0.35));
    p.rellenar_redondeado(
        encoger(barra, -1.0 * s),
        (d::RADIO_FLOTANTE + 1.0) * s,
        blanco(0.12),
    );
    p.rellenar_redondeado(barra, d::RADIO_FLOTANTE * s, d::CAJA);
    e.botones.zona(barra, Accion::Nada);
    let n = e.elegidas.len();
    let hay = n > 0;
    let lado = d::BOTON * s;
    let yb = barra.y + (alto - lado) / 2.0;

    let mut x = barra.x + barra.ancho - 10.0 * s - lado;
    d::boton_icono(
        p,
        &mut e.botones,
        RectF {
            x,
            y: yb,
            ancho: lado,
            alto: lado,
        },
        Accion::SalirEligiendo,
        &mi::CLOSE,
        false,
        false,
        s,
    );

    // De derecha a izquierda, como se colocan.
    let botones = [
        BotonBarra {
            accion: Accion::BarraBorrar,
            icono: &mi::DELETE,
            rotulo: textos.t("galeria-borrar"),
            chapa: "Supr",
            fondo: None,
            tinta: d::ROJO_TEXTO,
        },
        BotonBarra {
            accion: Accion::BarraGuardar,
            icono: &mi::IOS_SHARE,
            rotulo: textos.t("galeria-guardar-como"),
            chapa: "",
            fondo: Some(BOTON_BARRA),
            tinta: d::TEXTO,
        },
        BotonBarra {
            accion: Accion::BarraCopiar,
            icono: &mi::CONTENT_COPY,
            rotulo: textos.t("galeria-copiar"),
            chapa: "Ctrl C",
            fondo: Some(BOTON_BARRA),
            tinta: d::TEXTO,
        },
        BotonBarra {
            accion: Accion::BarraConservar,
            icono: &mi::BOOKMARK_ADD,
            rotulo: textos.t("galeria-conservar"),
            chapa: "Ctrl S",
            fondo: Some(BOTON_BARRA),
            tinta: d::VERDE,
        },
        BotonBarra {
            accion: Accion::BarraPinear,
            icono: &mi::PUSH_PIN,
            rotulo: textos.t("galeria-pinear"),
            chapa: "Intro",
            fondo: Some(d::AZUL_LLENO),
            tinta: Color::BLANCO,
        },
    ];
    let chapa = |b: &BotonBarra, con: bool| (con && !b.chapa.is_empty()).then_some(b.chapa);
    let anchos = |con: bool| -> f32 {
        botones
            .iter()
            .map(|b| crate::lecciones::ui::ancho_de_boton(p, true, &b.rotulo, chapa(b, con), s))
            .sum::<f32>()
            + d::HUECO * s * botones.len() as f32
            + 9.0 * s
    };
    // A la izquierda se quiere sitio para «3 elegidas» y «Todas»: si no
    // cabe todo, fuera las chapitas de los botones (los atajos siguen).
    let cuantas = con_cuantas(textos, "galeria-elegidas", n);
    let tam = d::LETRA_TITULO_TARJETA * s;
    let (cw, ch) = crate::lecciones::ui::medir_negrita(p, &cuantas, tam, 10_000.0);
    let todas = textos.t("galeria-elegir-todas");
    let ancho_todas = crate::lecciones::ui::ancho_de_boton(p, false, &todas, Some("Ctrl A"), s);
    let izquierda = 16.0 * s + cw + 10.0 * s + ancho_todas + 16.0 * s;
    let con_chapas = izquierda + anchos(true) < x - barra.x;
    for (k, b) in botones.iter().enumerate() {
        let wb = crate::lecciones::ui::ancho_de_boton(p, true, &b.rotulo, chapa(b, con_chapas), s);
        x -= d::HUECO * s + wb;
        let caja = RectF {
            x,
            y: yb,
            ancho: wb,
            alto: lado,
        };
        if hay {
            boton_v2(
                p,
                &mut e.botones,
                caja,
                b.accion,
                Some(b.icono),
                &b.rotulo,
                chapa(b, con_chapas),
                b.fondo,
                b.tinta,
                s,
            );
        } else {
            // Sin ninguna elegida, apagados: atenuados y sin hacer nada
            // (con la barra vacia, «Borrar» actuaria sobre el foco).
            boton_v2(
                p,
                &mut e.botones,
                caja,
                Accion::Nada,
                Some(b.icono),
                &b.rotulo,
                chapa(b, con_chapas),
                Some(blanco(0.05)),
                blanco(0.35),
                s,
            );
        }
        if k == 0 {
            // La raya que aparta «Borrar» de lo demas.
            x -= 9.0 * s;
            p.rellenar(
                RectF {
                    x: x + 4.0 * s,
                    y: barra.y + (alto - 28.0 * s) / 2.0,
                    ancho: 1.0 * s,
                    alto: 28.0 * s,
                },
                blanco(0.14),
            );
        }
    }
    // A la izquierda, cuantas y «Todas».
    let xi = barra.x + 16.0 * s;
    if xi + cw < x - 8.0 * s {
        crate::lecciones::ui::negrita(
            p,
            &cuantas,
            xi,
            barra.y + (alto - ch) / 2.0,
            tam,
            10_000.0,
            d::TEXTO,
        );
    }
    let xt = xi + cw + 10.0 * s;
    if xt + ancho_todas < x - 8.0 * s {
        boton_v2(
            p,
            &mut e.botones,
            RectF {
                x: xt,
                y: yb,
                ancho: ancho_todas,
                alto: lado,
            },
            Accion::ElegirTodas,
            None,
            &todas,
            Some("Ctrl A"),
            None,
            d::ACENTO,
            s,
        );
    }
    barra
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
    fn una_captura_nueva_se_llama_por_su_hora_y_no_reutiliza_nombres() {
        let dir = temporal("nombre-unico");
        // 2026-10-09 14:25:07 (hora local ya corrida).
        let t = 1_791_555_907_000;
        let a = ruta_nueva_en(&dir, t);
        assert_eq!(
            a.file_name().unwrap().to_string_lossy(),
            "captura-20261009-142507.png"
        );
        assert!(es_captura(&a));
        // En el mismo segundo, otra: no pisa la primera (reservada o GIF).
        std::fs::write(&a, b"").unwrap();
        let b = ruta_nueva_en(&dir, t + 300);
        assert!(b.ends_with("captura-20261009-142507-2.png"));
        std::fs::write(dir.join("captura-20261009-142507-2.mp4"), b"x").unwrap();
        assert!(ruta_nueva_en(&dir, t).ends_with("captura-20261009-142507-3.png"));
        // Caso negativo: borrar una captura vieja no deja su nombre libre
        // para la siguiente (antes, `captura-0001` volvia a salir).
        std::fs::write(dir.join("captura-0001.png"), b"x").unwrap();
        std::fs::remove_file(dir.join("captura-0001.png")).unwrap();
        let c = ruta_nueva_en(&dir, t + 1000);
        assert!(c.ends_with("captura-20261009-142508.png"));
        // Un 29 de febrero y el cambio de ano, por la cuenta de fechas.
        assert_eq!(nombre_por_hora(951_782_400_000), "captura-20000229-000000");
        assert_eq!(nombre_por_hora(1_798_761_599_000), "captura-20261231-235959");
        let _ = std::fs::remove_dir_all(&dir);
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
            ..Default::default()
        };
        let mut e = Estado::nuevo(lista, registro);
        e.ocr = Some(true);
        e.leidos.insert(
            e.lista[0].ruta.clone(),
            logica::plegar("Presupuesto Obra Miraflores"),
        );
        e
    }

    fn ubicacion() -> Ubicacion {
        Ubicacion::Portable {
            raiz: std::env::temp_dir().join(format!("pixpin-galeria-ub-{}", std::process::id())),
        }
    }

    #[test]
    fn sin_filtros_se_ven_todas_y_se_busca_por_el_texto_de_dentro() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        // Todas: la conservada, el video y la vieja tambien. Ya no hay
        // filtro que las aparte.
        assert_eq!(e.vista, vec![0, 1, 2, 3]);
        assert!(
            e.foco.is_none(),
            "caso negativo: nadie eligio ninguna, asi que no hay foco"
        );
        // El texto reconocido no se ensena, pero se sigue buscando por el.
        e.consulta = Campo::con("miraflores");
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![0], "encuentra por el texto reconocido");
        e.consulta = Campo::con("0001");
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![3], "y por el nombre");
        // Caso negativo: lo que no esta en ninguna, nada.
        e.consulta = Campo::con("ladrillo");
        preparar(&mut e, 684.0, 1.0, &textos);
        assert!(e.vista.is_empty());
    }

    #[test]
    fn al_abrir_intro_y_supr_no_tocan_una_captura_que_nadie_eligio() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let z = zonas(1280.0, 820.0, 1.0);
        // Caso negativo: sin foco, ni Supr borra ni Intro pinea ni Ctrl+C
        // copia; no sale ni el aviso.
        tecla(&mut e, 0x2E, false, z, 1.0, &textos, &ub, &mut vivo);
        tecla(&mut e, 0x0D, false, z, 1.0, &textos, &ub, &mut vivo);
        tecla(&mut e, u32::from(b'C'), true, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.lista.len(), 4);
        assert!(e.deshacer.is_empty() && e.aviso.is_none());
        assert!(e.objetivos().is_empty());
        // Una flecha pone el foco en la primera: desde ahi si hay objetivo.
        tecla(&mut e, 0x27, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.objetivos(), vec![0]);
    }

    #[test]
    fn escribir_va_al_buscador_y_el_espacio_elige() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let z = zonas(1280.0, 820.0, 1.0);
        // Caso negativo: sin foco, el espacio no elige nada.
        tecla(&mut e, 0x20, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!e.eligiendo && e.elegidas.is_empty());
        // Con el foco puesto (una flecha), el espacio elige la del foco.
        tecla(&mut e, 0x27, false, z, 1.0, &textos, &ub, &mut vivo);
        caracter(&mut e, ' ');
        tecla(&mut e, 0x20, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(e.eligiendo && e.elegidas.contains(&e.lista[0].ruta));
        assert!(
            e.consulta.texto.is_empty(),
            "caso negativo: el espacio no se escribe"
        );
        // Una letra enfoca el buscador y se escribe.
        caracter(&mut e, 'm');
        caracter(&mut e, 'i');
        assert!(e.escribiendo);
        assert_eq!(e.consulta.texto, "mi");
        // Ahi el espacio si se escribe, y Retroceso borra.
        caracter(&mut e, ' ');
        tecla(&mut e, 0x08, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.consulta.texto, "mi");
        // Esc limpia; otro Esc sale del buscador; otro sale de elegir; y el
        // ultimo cierra.
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(e.consulta.texto.is_empty() && e.escribiendo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!e.escribiendo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!e.eligiendo && e.elegidas.is_empty() && vivo);
        tecla(&mut e, 0x1B, false, z, 1.0, &textos, &ub, &mut vivo);
        assert!(!vivo);
        // Caso negativo: Ctrl+C no escribe una «c» (llega como control).
        caracter(&mut e, '\u{3}');
        assert!(e.consulta.texto.is_empty());
    }

    #[test]
    fn el_buscador_es_una_caja_de_verdad_con_cursor_y_ctrl_retroceso() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        let z = zonas(1280.0, 820.0, 1.0);
        for c in "obra lima".chars() {
            caracter(&mut e, c);
        }
        // La flecha izquierda mueve el cursor (antes solo se podia borrar
        // desde el final) y lo escrito entra donde esta.
        for _ in 0..4 {
            tecla(&mut e, 0x25, false, z, 1.0, &textos, &ub, &mut vivo);
        }
        caracter(&mut e, 'X');
        assert_eq!(e.consulta.texto, "obra Xlima");
        // Ctrl+Retroceso borra la palabra de antes del cursor, no todo.
        tecla(&mut e, 0x08, true, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.consulta.texto, "obra lima");
        // Caso negativo: las flechas de la caja no mueven el foco de la
        // rejilla mientras se escribe.
        assert!(e.foco.is_none());
    }

    #[test]
    fn las_flechas_mueven_el_foco_y_ctrl_a_elige_todas_las_que_se_ven() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let z = zonas(1280.0, 820.0, 1.0);
        // La primera flecha pone el foco en la primera; la siguiente lo mueve.
        tecla(&mut e, 0x27, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(0));
        tecla(&mut e, 0x27, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(1));
        // Abajo: de la fila de hoy al grupo de hace diez dias.
        tecla(&mut e, 0x28, false, z, 1.0, &textos, &ub, &mut vivo);
        assert_eq!(e.foco_i(), Some(3));
        // Una busqueda que la deja fuera: el foco se suelta (no salta a
        // otra que nadie eligio).
        e.consulta = Campo::con("presupuesto");
        preparar(&mut e, 684.0, 1.0, &textos);
        assert_eq!(e.vista, vec![0]);
        assert_eq!(e.foco_i(), None, "el foco se va de la que ya no se ve");
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
        // Caso negativo: las que la busqueda esconde no se eligen.
        assert_eq!(e.elegidas.len(), 1);
        assert_eq!(e.objetivos(), vec![0]);
        // Otra vez Ctrl+A las suelta; sin elegidas ni foco, no hay objetivo.
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
        assert!(e.objetivos().is_empty());
    }

    #[test]
    fn el_clic_derecho_y_el_boton_de_mas_abren_el_menu_de_la_captura_de_debajo() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ub = ubicacion();
        let mut vivo = true;
        let mut e = estado();
        preparar(&mut e, 684.0, 1.0, &textos);
        let celda = RectF {
            x: 20.0,
            y: 100.0,
            ancho: 200.0,
            alto: 140.0,
        };
        e.botones.zona(celda, Accion::Celda(2));
        e.botones.raton = (60.0, 150.0);
        assert!(clic_derecho(&mut e));
        assert_eq!((e.menu_pendiente, e.foco_i()), (Some(2), Some(2)));
        // El «⋯» abre el mismo menu.
        e.menu_pendiente = None;
        hacer(&mut e, Accion::Mas(1), false, &textos, &ub, &mut vivo);
        assert_eq!((e.menu_pendiente, e.foco_i()), (Some(1), Some(1)));
        // Caso negativo: fuera de toda captura no sale menu.
        e.menu_pendiente = None;
        e.botones.raton = (600.0, 600.0);
        assert!(!clic_derecho(&mut e));
        assert_eq!(e.menu_pendiente, None);
    }

    #[test]
    fn conservar_lo_ya_conservado_avisa_en_vez_de_callar() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let raiz = temporal("ya-conservadas");
        let mut e = estado();
        // La 1 (captura-0002) ya esta conservada.
        conservar_varias(&mut e, &[1], &raiz, &textos);
        assert_eq!(
            e.aviso.as_ref().map(|a| a.texto.as_str()),
            Some("Ya estaban conservadas")
        );
        // Caso negativo: sin nada pedido no hay aviso.
        e.aviso = None;
        conservar_varias(&mut e, &[], &raiz, &textos);
        assert!(e.aviso.is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_chapita_dice_hoy_el_ultimo_dia_y_su_pista_explica_que_hacer() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        assert_eq!(rotulo_de_dias(&textos, 0), "hoy");
        assert_eq!(rotulo_de_dias(&textos, 6), "6");
        let pista = pista_de_dias(&textos, 6);
        assert!(pista.contains("6 días") && pista.contains("clic derecho"), "{pista}");
        assert!(pista_de_dias(&textos, 0).contains("hoy"));
        assert!(pista_de_dias(&textos, 1).contains("mañana"));
        // Caso negativo: un numero negativo (reloj atrasado) no sale «-1».
        assert_eq!(rotulo_de_dias(&textos, -1), "hoy");
    }

    #[test]
    fn el_aviso_de_borrar_lleva_deshacer_y_dura_mas() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let raiz = temporal("aviso-borrar");
        let dir = carpeta_en(&raiz);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("captura-0001.png");
        std::fs::write(&a, b"x").unwrap();
        let registro = crate::caducidad_capturas::Registro {
            desde: 0,
            ..Default::default()
        };
        let mut e = Estado::nuevo(listar(&dir), registro);
        borrar_varias(&mut e, &[0], &raiz, &textos);
        let aviso = e.aviso.as_ref().expect("avisa");
        assert_eq!(
            aviso.accion.as_ref().map(|x| x.accion),
            Some(Accion::Deshacer)
        );
        assert_eq!(aviso.dura(), d::aviso::DURA_CON_ACCION);
        // Caso negativo: copiar sin nada no avisa de nada.
        e.aviso = None;
        copiar(&mut e, &[], &textos);
        assert!(e.aviso.is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sin_carril_ni_panel_la_rejilla_es_toda_la_ventana_bajo_la_cabecera() {
        let z = zonas(1280.0, 820.0, 1.0);
        assert_eq!(z.rejilla.x, 0.0, "nada a su izquierda");
        assert_eq!(z.rejilla.ancho, 1280.0, "nada a su derecha");
        assert_eq!(z.rejilla.y + z.rejilla.alto, 820.0);
        // Con escala 1,5 todo crece igual.
        let z2 = zonas(1920.0, 1230.0, 1.5);
        assert_eq!(z2.rejilla.y, logica::CABECERA * 1.5);
        // Caso negativo: una ventana mas baja que la cabecera no da alto
        // negativo.
        assert_eq!(zonas(700.0, 10.0, 1.0).rejilla.alto, 0.0);
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

    /// Una «captura de pantalla» de ejemplo: una ventana de un color, con su
    /// barra de titulo y unas lineas de texto, para que la muestra se lea
    /// como la galeria de verdad y no como cuadros grises.
    fn mini_de_ejemplo(k: usize) -> ImagenRgba {
        const TONOS: [[u8; 3]; 8] = [
            [236, 239, 244],
            [30, 34, 44],
            [250, 246, 238],
            [22, 58, 96],
            [244, 244, 246],
            [40, 44, 52],
            [232, 245, 236],
            [255, 250, 230],
        ];
        const ACENTOS: [[u8; 3]; 6] = [
            [10, 132, 255],
            [255, 159, 10],
            [48, 209, 88],
            [191, 90, 242],
            [255, 69, 58],
            [100, 210, 255],
        ];
        let (w, h) = (256u32, 160u32);
        let fondo = TONOS[k % TONOS.len()];
        let acento = ACENTOS[k % ACENTOS.len()];
        let oscuro = fondo.iter().map(|&c| u32::from(c)).sum::<u32>() < 300;
        let tinta: [u8; 3] = if oscuro { [150, 156, 170] } else { [120, 124, 132] };
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let c = if y < 14 {
                    // La barra de titulo, con sus tres botones.
                    if y > 4 && y < 10 && (x % 10 < 6) && x > 222 {
                        tinta
                    } else if oscuro {
                        [fondo[0] / 2, fondo[1] / 2, fondo[2] / 2]
                    } else {
                        [fondo[0] - 14, fondo[1] - 14, fondo[2] - 14]
                    }
                } else if x < 54 && k % 3 != 2 {
                    // Una barra lateral.
                    if (y - 14) % 18 < 8 && x > 8 && x < 46 && y > 24 {
                        tinta
                    } else if oscuro {
                        [fondo[0] + 10, fondo[1] + 10, fondo[2] + 10]
                    } else {
                        [fondo[0] - 8, fondo[1] - 8, fondo[2] - 8]
                    }
                } else if y > 26 && y < 40 && x > 66 && x < 66 + 60 + (k as u32 * 23) % 110 {
                    acento
                } else if y > 48 && (y - 48) % 12 < 5 && x > 66 && x < 240 - ((y * 7 + k as u32 * 13) % 70) {
                    tinta
                } else {
                    fondo
                };
                px.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        ImagenRgba {
            ancho: w,
            alto: h,
            pixeles: px,
        }
    }

    /// Una galeria de ejemplo: 25 capturas en cuatro dias, dos conservadas
    /// y una que se va hoy, con su miniatura cada una.
    fn estado_de_ejemplo() -> Estado {
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let seg = (ahora / 1000) as u64;
        let mut lista = Vec::new();
        let mut n = 300;
        for (dia, cuantas) in [(0u64, 9u64), (1, 6), (7, 6), (9, 4)] {
            for k in 0..cuantas {
                let ext = if n % 11 == 0 {
                    "gif"
                } else if n % 13 == 0 {
                    "mp4"
                } else {
                    "png"
                };
                lista.push(entrada(
                    &format!("captura-{n:04}.{ext}"),
                    seg - dia * 86_400 - k * 900 - 60,
                ));
                n -= 1;
            }
        }
        ordenar(&mut lista);
        let registro = crate::caducidad_capturas::Registro {
            desde: ahora - 30 * 86_400_000,
            conservadas: ["captura-0298.png", "captura-0285.png"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        };
        let mut e = Estado::nuevo(lista, registro);
        e.ocr = Some(true);
        for (k, x) in e.lista.iter().enumerate() {
            if tiene_miniatura(&x.ruta) {
                e.minis.insert(
                    x.ruta.clone(),
                    Mini::Lista {
                        bitmap: None,
                        imagen: mini_de_ejemplo(k),
                        visto: 0,
                    },
                );
            }
        }
        e
    }

    /// Sube a la GPU las miniaturas de ejemplo (dentro del pintado de la
    /// muestra: es el unico sitio donde hay motor).
    fn subir_minis(e: &mut Estado, motor: &pixpin_render::MotorRender) {
        for m in e.minis.values_mut() {
            if let Mini::Lista { bitmap, imagen, .. } = m
                && bitmap.is_none()
            {
                *bitmap = motor
                    .bitmap_desde_pixeles(imagen.ancho, imagen.alto, &imagen.pixeles)
                    .ok();
            }
        }
    }

    /// El centro de la celda `k` de la vista, en pixeles de la ventana.
    fn sobre_la_celda(e: &Estado, k: usize, dx: f32, dy: f32) -> (f32, f32) {
        let z = zonas(1280.0, 820.0, 1.0);
        let c = e.disp.celdas[k];
        (z.rejilla.x + c.x + dx, z.rejilla.y + c.y - e.scroll + dy)
    }

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_galeria() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let ancho_rejilla = zonas(1280.0, 820.0, 1.0).rejilla.ancho;

        // 1. Sin elegir: una con el foco (se pulso) y el raton sobre la
        // chapita de otra, que ensena el «⋯» y la pista.
        let mut e = estado_de_ejemplo();
        preparar(&mut e, ancho_rejilla, 1.0, &textos);
        e.foco = Some(e.lista[e.vista[2]].ruta.clone());
        let c = e.disp.celdas[0];
        e.botones.raton = sobre_la_celda(&e, 0, c.ancho - 17.0, 17.0);
        crate::ventanita::muestra("galeria-v2-encima", 1280, 820, |p, motor| {
            subir_minis(&mut e, motor);
            pintar_todo(&mut e, p, 1280.0, 820.0, 1.0, &textos);
        });

        // 2. Eligiendo varias, con el aviso de borrar y su «Deshacer»
        // encima de la barra, y el raton sobre una celda (su «⋯»).
        let mut e = estado_de_ejemplo();
        preparar(&mut e, ancho_rejilla, 1.0, &textos);
        e.eligiendo = true;
        for k in [0, 2, 3] {
            e.elegidas.insert(e.lista[e.vista[k]].ruta.clone());
        }
        e.aviso = Some(Aviso::con_deshacer(
            con_cuantas(&textos, "galeria-borradas", 2),
            textos.t("galeria-deshacer"),
            Accion::Deshacer,
        ));
        e.deshacer = vec![Borrada {
            original: PathBuf::from("x.png"),
            destino: PathBuf::from("y.png"),
            conservada: false,
            prorroga: None,
        }];
        e.botones.raton = sobre_la_celda(&e, 7, 60.0, 70.0);
        crate::ventanita::muestra("galeria-v2-eligiendo", 1280, 820, |p, motor| {
            subir_minis(&mut e, motor);
            pintar_todo(&mut e, p, 1280.0, 820.0, 1.0, &textos);
        });

        // 3. Buscando: el cursor en medio de lo escrito, y una busqueda sin
        // nada (el estado vacio).
        let mut e = estado_de_ejemplo();
        e.escribiendo = true;
        e.consulta = Campo::con("ladrillo");
        e.consulta.cursor = 3;
        preparar(&mut e, ancho_rejilla, 1.0, &textos);
        crate::ventanita::muestra("galeria-v2-buscar", 1280, 820, |p, motor| {
            subir_minis(&mut e, motor);
            pintar_todo(&mut e, p, 1280.0, 820.0, 1.0, &textos);
        });
    }
}
