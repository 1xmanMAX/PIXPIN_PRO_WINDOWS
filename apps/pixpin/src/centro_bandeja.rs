//! **El panel de la bandeja** (rediseno v2, `Bandeja2.dc.html`): lo que
//! sale al hacer clic IZQUIERDO en el icono de PixPin junto al reloj. El
//! menu del clic derecho sigue igual, de respaldo, con sus numeros de
//! siempre.
//!
//! Lleva, de arriba abajo: el estado de sincronizar y «Sincronizar ahora»,
//! un buscador de acciones, «Capturar» grande con sus modos, los favoritos
//! que elige el usuario (se guardan en `[bandeja] favoritos` del fichero de
//! ajustes, ver `pixpin_store::bandeja`), los interruptores de los pines y
//! los atajos, las ultimas capturas con sus acciones, las ventanas (abrir
//! PixPin, Tareas, Lecciones, documento; la galeria en «Ver la galeria») y
//! «Salir», apartado y en rojo.
//!
//! ## Como hace las cosas
//!
//! No las hace: cada boton manda al hilo principal el mismo `WM_COMMAND`
//! que la entrada del menu de la bandeja ([`acciones::Accion::id`]). Capturar,
//! ocultar los pines o salir siguen pasando por el bucle de `main.rs`, que es
//! quien tiene los pines, los atajos y el overlay. Lo unico que el panel
//! hace por su cuenta es lo de sus miniaturas (copiar, pinear, borrar con
//! «Deshacer»), como la galeria.
//!
//! El estado de los interruptores lo publica el hilo principal en cada
//! vuelta de su bucle ([`publicar`]); el panel solo lo lee.
//!
//! ## Ligero
//!
//! Vive en su propio hilo, que se queda un minuto tras cerrarse (abrirlo
//! otra vez es instantaneo) y luego se va con su dispositivo de dibujo. Las
//! miniaturas y los numeros de Tareas y Lecciones se leen en otro hilo.
//!
//! ## Teclado (solo con el panel delante)
//!
//! | Tecla | Que hace |
//! |---|---|
//! | Escribir | Busca una accion |
//! | ↑ ↓ / Intro | Elegir y hacer el resultado; sin buscar, Intro captura |
//! | Esc | Borra lo buscado, sale de elegir favoritos y al final cierra |
//! | Ctrl C / Supr | Copiar / borrar la captura bajo el raton |
//! | Ctrl Z | Deshacer el borrado |

#![forbid(unsafe_code)]

mod acciones;
mod disposicion;
mod iconos;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::letras::{Letra, SIN_PARTIR};
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::comandos::{self, Comando};
use pixpin_store::{Ajustes, Catalogo, Idioma, Ubicacion};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use acciones::Accion;
use disposicion::{AccionMiniatura, Disposicion, Modo, Vista, Zona};

// ------------------------------------------------------------- el estado

/// Lo que el panel ensena de los interruptores. Lo sabe el hilo principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Interruptores {
    /// Pines en pantalla ahora.
    pub a_la_vista: usize,
    /// «Ocultar los pines» encendido.
    pub ocultos: bool,
    /// Los pines dejan pasar el clic.
    pub pasantes: bool,
    /// Los atajos globales estan soltados.
    pub silenciados: bool,
}

static ESTADO: Mutex<Interruptores> = Mutex::new(Interruptores {
    a_la_vista: 0,
    ocultos: false,
    pasantes: false,
    silenciados: false,
});

/// La ventana del panel (> 0), -1 mientras nace, 0 sin hilo.
static VENTANA: AtomicIsize = AtomicIsize::new(0);
/// Si se ve ahora.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Que se cierre (otro clic en el icono con el panel abierto).
static CERRAR: AtomicBool = AtomicBool::new(false);
/// Cuando se cerro por perder el foco. El clic en el icono que lo cerro
/// llega DESPUES al hilo principal, y sin esto lo volveria a abrir.
static CERRADO_POR_FOCO_MS: AtomicI64 = AtomicI64::new(0);
/// El pedido de abrir que espera al hilo.
static PEDIDO: Mutex<Option<Pedido>> = Mutex::new(None);

/// Cuanto vale el «lo acabo de cerrar» tras perder el foco.
const GRACIA_MS: i64 = 400;
/// Cuanto se queda el hilo, con el panel cerrado, antes de irse.
const VIVE_CERRADO: Duration = Duration::from_secs(60);

struct Pedido {
    idioma: Idioma,
    ubicacion: Ubicacion,
    /// El atajo de cada comando que lo tiene, como chapita («Ctrl Alt X»).
    atajos: HashMap<u32, String>,
    raton: Punto,
}

/// El hilo principal dice como estan los interruptores. Se llama en cada
/// vuelta de su bucle: si nada cambio no cuesta mas que mirar un candado.
pub fn publicar(nuevo: Interruptores) {
    let cambio = match ESTADO.lock() {
        Ok(mut g) if *g != nuevo => {
            *g = nuevo;
            true
        }
        _ => false,
    };
    if cambio && VISIBLE.load(Ordering::SeqCst) {
        despertar();
    }
}

fn estado() -> Interruptores {
    ESTADO.lock().map(|g| *g).unwrap_or_default()
}

fn despertar() {
    let h = VENTANA.load(Ordering::SeqCst);
    if h > 0 {
        pixpin_shell::overlay::despertar(h);
    }
}

/// **Clic izquierdo en el icono**: abre el panel, o lo cierra si ya estaba.
/// Vuelve enseguida.
pub fn abrir(idioma: Idioma, ubicacion: &Ubicacion, config: &Ajustes) {
    if VISIBLE.load(Ordering::SeqCst) {
        CERRAR.store(true, Ordering::SeqCst);
        despertar();
        return;
    }
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    if ahora - CERRADO_POR_FOCO_MS.load(Ordering::SeqCst) < GRACIA_MS {
        return;
    }
    let (enlaces, _) = comandos::Enlaces::de_ajustes(config);
    let atajos = comandos::CATALOGO
        .iter()
        .filter_map(|d| {
            enlaces
                .atajo_de(d.comando)
                .map(|a| (d.comando.id(), acciones::chapita(&a.to_string())))
        })
        .collect();
    if let Ok(mut g) = PEDIDO.lock() {
        *g = Some(Pedido {
            idioma,
            ubicacion: ubicacion.clone(),
            atajos,
            raton: pixpin_shell::entorno::posicion_del_cursor(),
        });
    }
    match VENTANA.load(Ordering::SeqCst) {
        0 => lanzar(),
        h if h > 0 => pixpin_shell::overlay::despertar(h),
        // Naciendo: recogera el pedido en cuanto tenga ventana.
        _ => {}
    }
}

fn tomar_pedido() -> Option<Pedido> {
    PEDIDO.lock().ok().and_then(|mut g| g.take())
}

fn lanzar() {
    VENTANA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("panel-bandeja".into())
        .spawn(|| {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) = vivir() {
                tracing::warn!(?e, "el panel de la bandeja no pudo abrirse");
            }
            VISIBLE.store(false, Ordering::SeqCst);
            VENTANA.store(0, Ordering::SeqCst);
            // Un pedido que llego mientras el hilo se iba no se pierde.
            if PEDIDO.lock().map(|g| g.is_some()).unwrap_or(false) {
                lanzar();
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del panel de la bandeja");
        VENTANA.store(0, Ordering::SeqCst);
    }
}

// --------------------------------------------------------------- colores

/// Los colores de la maqueta (oscuro) y su pareja clara, segun el tema de
/// Windows.
#[derive(Debug, Clone, Copy)]
struct Paleta {
    fondo: Color,
    borde: Color,
    carta: Color,
    carta_encima: Color,
    texto: Color,
    suave: Color,
    suave2: Color,
    azul: Color,
    azul_encima: Color,
    azul_claro: Color,
    enlace: Color,
    verde: Color,
    rojo: Color,
    naranja: Color,
    amarillo: Color,
    apagado: Color,
    velo: Color,
    chapa: Color,
}

const fn alfa(c: Color, a: f32) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

impl Paleta {
    fn oscura() -> Paleta {
        Paleta {
            fondo: hex(0x1C1C1E),
            borde: alfa(Color::BLANCO, 0.10),
            carta: hex(0x2C2C2E),
            carta_encima: hex(0x3A3A3C),
            texto: hex(0xF5F5F7),
            suave: hex(0x98989D),
            suave2: hex(0xC7C7CC),
            azul: hex(0x0060DF),
            azul_encima: hex(0x1A73F0),
            azul_claro: hex(0x0A84FF),
            enlace: hex(0x64D2FF),
            verde: hex(0x30D158),
            rojo: hex(0xFF6961),
            naranja: hex(0xFF9F0A),
            amarillo: hex(0xFFD60A),
            apagado: hex(0x48484A),
            velo: alfa(Color::BLANCO, 0.06),
            chapa: alfa(Color::BLANCO, 0.09),
        }
    }

    fn clara() -> Paleta {
        Paleta {
            fondo: hex(0xF2F2F7),
            borde: alfa(Color::NEGRO, 0.12),
            carta: hex(0xFFFFFF),
            carta_encima: hex(0xE5E5EA),
            texto: hex(0x1C1C1E),
            suave: hex(0x6E6E73),
            suave2: hex(0x3A3A3C),
            azul: hex(0x0060DF),
            azul_encima: hex(0x1A73F0),
            azul_claro: hex(0x0A84FF),
            enlace: hex(0x0066CC),
            verde: hex(0x34C759),
            rojo: hex(0xD70015),
            naranja: hex(0xC93400),
            amarillo: hex(0xB25000),
            apagado: hex(0xD1D1D6),
            velo: alfa(Color::NEGRO, 0.05),
            chapa: alfa(Color::NEGRO, 0.06),
        }
    }
}

// ------------------------------------------------------- lo que llega

/// Lo que manda el hilo de lectura.
enum Llegada {
    Insignias { tareas: usize, lecciones: usize },
    Miniatura(PathBuf, Option<ImagenRgba>),
}

enum Mini {
    Pedida,
    Lista {
        bitmap: Option<ID2D1Bitmap1>,
        imagen: ImagenRgba,
    },
    Imposible,
}

/// Lado mayor de la miniatura reducida (como la galeria).
const LADO: u32 = 256;

/// Lee, en otro hilo, los numeros de Tareas y Lecciones y las miniaturas.
fn leer_en_otro_hilo(raiz: PathBuf, capturas: Vec<PathBuf>, hwnd: isize) -> Receiver<Llegada> {
    let (tx, rx) = channel();
    let lanzado = std::thread::Builder::new()
        .name("panel-bandeja-lector".into())
        .spawn(move || {
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let tareas: usize = crate::tareas::reunir(&raiz)
                .0
                .iter()
                .filter(|l| crate::tareas::es_inbox(l))
                .map(|l| l.cuantas_pendientes())
                .sum();
            let lecciones = crate::lecciones::almacen::listar(&raiz)
                .iter()
                .filter(|e| pixpin_lecciones::leccion::Repaso::toca(&e.leccion, ahora))
                .count();
            if tx.send(Llegada::Insignias { tareas, lecciones }).is_err() {
                return;
            }
            pixpin_shell::overlay::despertar(hwnd);
            for ruta in capturas {
                let mini = if es_video(&ruta) {
                    None
                } else {
                    reducida(&ruta)
                };
                if tx.send(Llegada::Miniatura(ruta, mini)).is_err() {
                    return;
                }
                pixpin_shell::overlay::despertar(hwnd);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el lector del panel de la bandeja");
    }
    rx
}

fn es_video(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

fn reducida(ruta: &Path) -> Option<ImagenRgba> {
    let entera = pixpin_codec::imagen::cargar(ruta).ok()?;
    let (w, h) = (entera.ancho, entera.alto);
    if w == 0 || h == 0 {
        return None;
    }
    let mayor = w.max(h);
    if mayor <= LADO {
        return Some(entera);
    }
    let f = LADO as f64 / mayor as f64;
    let nw = ((w as f64 * f).round() as u32).max(1);
    let nh = ((h as f64 * f).round() as u32).max(1);
    pixpin_codec::redimensionar(entera, nw, nh).ok()
}

/// La ultima vez que se sincronizo con algun aparato: lo que apunta la
/// sincronizacion al acabar (`sincro/elegidos/<id>.cuando`, en ms). 0 si
/// nunca.
pub fn ultima_sincro(raiz: &Path) -> i64 {
    let Ok(lista) = std::fs::read_dir(raiz.join("sincro").join("elegidos")) else {
        return 0;
    };
    lista
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "cuando"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| t.trim().parse::<i64>().ok())
        .max()
        .unwrap_or(0)
}

/// «hace 2 min», «hace 3 h», «ayer»… para el estado de la cabecera.
fn hace_cuanto(textos: &Catalogo, cuando: i64, ahora: i64) -> String {
    let min = (ahora - cuando).max(0) / 60_000;
    let mut args = fluent_bundle::FluentArgs::new();
    match min {
        0 => textos.t("bandeja2-hace-nada"),
        1..=59 => {
            args.set("n", min);
            textos.t_args("bandeja2-hace-min", &args)
        }
        60..=1439 => {
            args.set("n", min / 60);
            textos.t_args("bandeja2-hace-h", &args)
        }
        1440..=2879 => textos.t("bandeja2-ayer"),
        _ => {
            args.set("n", min / 1440);
            textos.t_args("bandeja2-hace-dias", &args)
        }
    }
}

// ----------------------------------------------------------- el panel

/// Lo borrado, para devolverlo con «Deshacer».
struct Borrada {
    original: PathBuf,
    en_papelera: PathBuf,
    conservada: bool,
}

struct Aviso {
    texto: String,
    deshacer: Option<Borrada>,
    desde: Instant,
}

/// Los modos de la fila de «Capturar». La maqueta pone tambien «Ventana» y
/// «Pantalla»: la captura de zona ya engancha ventanas al pasar por encima
/// y Ctrl+A coge la pantalla entera, pero no hay comandos que entren asi
/// directamente, y un boton que no hace lo que dice no se pone.
const MODOS: [(Comando, &str); disposicion::MODOS] = [
    (Comando::CapturarRegion, "bandeja2-modo-zona"),
    (Comando::CapturarConScroll, "bandeja2-modo-scroll"),
    (Comando::GrabarGif, "bandeja2-modo-gif"),
    (Comando::CopiarTexto, "bandeja2-modo-texto"),
    (Comando::CapturarConRetardo, "bandeja2-modo-retardo"),
    (Comando::Cuentagotas, "bandeja2-modo-color"),
];

const INTERRUPTORES: [Comando; 3] = [
    Comando::AlternarPines,
    Comando::AlternarPasoDeClics,
    Comando::SilenciarAtajos,
];

const VENTANAS: [Accion; disposicion::VENTANAS] = [
    Accion::Comando(Comando::AbrirChat),
    Accion::Tareas,
    Accion::Lecciones,
    Accion::AbrirDocumento,
];

struct Panel {
    textos: Catalogo,
    ubicacion: Ubicacion,
    atajos: HashMap<u32, String>,
    paleta: Paleta,
    escala: f32,
    marco: Rect,
    consulta: String,
    editando: bool,
    elegida: usize,
    scroll: f32,
    favoritos: Vec<Accion>,
    titulos: Vec<(Accion, String)>,
    capturas: Vec<crate::galeria_capturas::Entrada>,
    minis: HashMap<PathBuf, Mini>,
    tareas: Option<usize>,
    lecciones: Option<usize>,
    ultima_sincro: i64,
    raton: (f32, f32),
    aviso: Option<Aviso>,
    /// Donde se pinto «Deshacer» (no es de la disposicion: va encima).
    boton_deshacer: Option<RectF>,
    llegadas: Receiver<Llegada>,
    /// Ya estuvo delante alguna vez: solo entonces perder el foco lo cierra.
    fue_delante: bool,
}

impl Panel {
    fn nuevo(p: Pedido, hwnd: isize) -> Result<Panel> {
        let textos = Catalogo::nuevo(p.idioma);
        let raiz = p.ubicacion.raiz().to_path_buf();
        let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let monitor = monitores
            .monitores()
            .iter()
            .find(|m| {
                let a = m.area;
                p.raton.x >= a.x
                    && p.raton.x < a.x + a.ancho as i32
                    && p.raton.y >= a.y
                    && p.raton.y < a.y + a.alto as i32
            })
            .or_else(|| monitores.principal())
            .context("sin monitor")?
            .to_owned();
        let escala = monitor.escala_por_cien as f32 / 100.0;
        let capturas: Vec<_> =
            crate::galeria_capturas::listar(&crate::galeria_capturas::carpeta(&p.ubicacion))
                .into_iter()
                .take(disposicion::MINIATURAS)
                .collect();
        let llegadas = leer_en_otro_hilo(
            raiz.clone(),
            capturas.iter().map(|c| c.ruta.clone()).collect(),
            hwnd,
        );
        let titulos = acciones::todas()
            .into_iter()
            .map(|a| (a, textos.t(a.clave_titulo())))
            .collect();
        let favoritos = acciones::favoritos_de(&pixpin_store::bandeja::leer(&p.ubicacion));
        let mut panel = Panel {
            paleta: if pixpin_shell::entorno::tema_claro() {
                Paleta::clara()
            } else {
                Paleta::oscura()
            },
            escala,
            marco: Rect {
                x: 0,
                y: 0,
                ancho: 1,
                alto: 1,
            },
            consulta: String::new(),
            editando: false,
            elegida: 0,
            scroll: 0.0,
            favoritos,
            titulos,
            minis: capturas
                .iter()
                .map(|c| (c.ruta.clone(), Mini::Pedida))
                .collect(),
            capturas,
            tareas: None,
            lecciones: None,
            ultima_sincro: ultima_sincro(&raiz),
            raton: (-1.0, -1.0),
            aviso: None,
            boton_deshacer: None,
            llegadas,
            fue_delante: false,
            atajos: p.atajos,
            ubicacion: p.ubicacion,
            textos,
        };
        let d = panel.disposicion_normal();
        panel.marco = disposicion::colocar(
            monitor.area,
            monitor.area_trabajo,
            p.raton,
            d.ancho.round() as u32,
            d.alto_total.round() as u32,
            (12.0 * escala).round() as i32,
        );
        Ok(panel)
    }

    fn titulo(&self, a: Accion) -> String {
        self.titulos
            .iter()
            .find(|(x, _)| *x == a)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| self.textos.t(a.clave_titulo()))
    }

    /// Las filas de la lista: lo buscado, o al elegir favoritos todas (o
    /// las buscadas).
    fn filas(&self) -> Vec<Accion> {
        if self.consulta.trim().is_empty() {
            if self.editando {
                acciones::todas()
            } else {
                Vec::new()
            }
        } else {
            acciones::buscar(&self.consulta, &self.titulos)
        }
    }

    fn modo(&self) -> Modo {
        if self.editando {
            Modo::Editar {
                candidatas: self.filas().len(),
            }
        } else if !self.consulta.trim().is_empty() {
            Modo::Busqueda {
                resultados: self.filas().len(),
            }
        } else {
            Modo::Normal
        }
    }

    fn ancho_sincronizar(&self) -> f32 {
        let n = self.textos.t("bandeja2-sincronizar").chars().count() as f32;
        (34.0 + n * 6.4).clamp(90.0, 170.0)
    }

    fn vista(&self, modo: Modo) -> Vista {
        Vista {
            escala: self.escala,
            scroll: self.scroll,
            modo,
            favoritos: self.favoritos.len(),
            con_anadir: self.favoritos.len() < pixpin_store::bandeja::TOPE,
            ancho_sincronizar: self.ancho_sincronizar(),
        }
    }

    fn disposicion_normal(&self) -> Disposicion {
        let mut v = self.vista(Modo::Normal);
        v.scroll = 0.0;
        disposicion::disponer(&v)
    }

    fn disposicion(&self) -> Disposicion {
        disposicion::disponer(&self.vista(self.modo()))
    }

    fn scroll_maximo(&self) -> f32 {
        let mut v = self.vista(self.modo());
        v.scroll = 0.0;
        (disposicion::disponer(&v).alto_total - self.marco.alto as f32).max(0.0)
    }

    /// La miniatura bajo el raton, si tiene captura.
    fn miniatura_viva(&self, d: &Disposicion) -> Option<usize> {
        d.miniaturas
            .iter()
            .position(|c| disposicion::dentro(*c, self.raton))
            .filter(|i| *i < self.capturas.len())
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some(Aviso {
            texto,
            deshacer: None,
            desde: Instant::now(),
        });
    }

    fn fallo(&mut self, motivo: String) {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("motivo", motivo);
        let t = self.textos.t_args("bandeja2-no-se-pudo", &args);
        self.avisar(t);
    }

    fn atender_llegadas(&mut self) -> bool {
        let mut algo = false;
        while let Ok(l) = self.llegadas.try_recv() {
            algo = true;
            match l {
                Llegada::Insignias { tareas, lecciones } => {
                    self.tareas = Some(tareas);
                    self.lecciones = Some(lecciones);
                }
                Llegada::Miniatura(ruta, Some(imagen)) => {
                    // El bitmap se sube antes de pintar (`subir`).
                    self.minis.insert(
                        ruta,
                        Mini::Lista {
                            bitmap: None,
                            imagen,
                        },
                    );
                }
                Llegada::Miniatura(ruta, None) => {
                    self.minis.insert(ruta, Mini::Imposible);
                }
            }
        }
        algo
    }

    /// Sube a la GPU las miniaturas que aun no estan. Fuera del fotograma:
    /// crear bitmaps a medio dibujo no se puede.
    fn subir(&mut self, motor: &MotorRender) {
        for m in self.minis.values_mut() {
            if let Mini::Lista {
                bitmap: b @ None,
                imagen,
            } = m
            {
                *b = motor
                    .bitmap_desde_pixeles(imagen.ancho, imagen.alto, &imagen.pixeles)
                    .ok();
            }
        }
    }

    fn guardar_favoritos(&mut self) {
        let nombres: Vec<String> = self
            .favoritos
            .iter()
            .map(|a| a.nombre().to_string())
            .collect();
        if let Err(e) = pixpin_store::bandeja::guardar(&self.ubicacion, &nombres) {
            tracing::warn!(?e, "no se pudieron guardar los favoritos de la bandeja");
            self.fallo(e.to_string());
        }
    }

    // --- las miniaturas

    fn copiar(&mut self, i: usize) {
        let Some(ruta) = self.capturas.get(i).map(|c| c.ruta.clone()) else {
            return;
        };
        let hecho = pixpin_codec::imagen::cargar(&ruta)
            .map_err(|e| e.to_string())
            .and_then(|img| pixpin_codec::copiar_imagen(&img).map_err(|e| e.to_string()));
        match hecho {
            Ok(()) => {
                let t = self.textos.t("bandeja2-copiada");
                self.avisar(t);
            }
            Err(m) => self.fallo(m),
        }
    }

    /// A la papelera de PixPin, con «Deshacer» (no se pregunta antes).
    fn borrar(&mut self, i: usize) {
        let Some(ruta) = self.capturas.get(i).map(|c| c.ruta.clone()) else {
            return;
        };
        let raiz = self.ubicacion.raiz().to_path_buf();
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let nombre = crate::caducidad_capturas::nombre(&ruta);
        let conservada = crate::caducidad_capturas::leer(&raiz, ahora)
            .conservadas
            .contains(&nombre);
        match crate::galeria_capturas::a_la_papelera(&raiz, &ruta) {
            Ok(destino) => {
                tracing::info!(ruta = %ruta.display(), "captura a la papelera desde la bandeja");
                // Como `galeria_capturas::borrar`: una conservada se olvida,
                // o una captura nueva que heredara el nombre naceria
                // conservada. «Deshacer» la vuelve a apuntar.
                if conservada
                    && let Err(e) = crate::caducidad_capturas::cambiar(&raiz, ahora, |r| {
                        r.conservadas.remove(&nombre);
                    })
                {
                    tracing::warn!(?e, "no se pudo olvidar la captura conservada");
                }
                self.capturas.remove(i);
                self.minis.remove(&ruta);
                self.aviso = Some(Aviso {
                    texto: self.textos.t("bandeja2-borrada"),
                    deshacer: Some(Borrada {
                        original: ruta,
                        en_papelera: destino,
                        conservada,
                    }),
                    desde: Instant::now(),
                });
            }
            Err(e) => self.fallo(e.to_string()),
        }
    }

    fn deshacer(&mut self) {
        let Some(b) = self.aviso.take().and_then(|a| a.deshacer) else {
            return;
        };
        if let Err(e) = std::fs::rename(&b.en_papelera, &b.original) {
            self.fallo(e.to_string());
            return;
        }
        let raiz = self.ubicacion.raiz().to_path_buf();
        if b.conservada {
            let nombre = crate::caducidad_capturas::nombre(&b.original);
            let _ = crate::caducidad_capturas::cambiar(
                &raiz,
                pixpin_shell::entorno::ahora_utc_ms(),
                |r| {
                    r.conservadas.insert(nombre);
                },
            );
        }
        // Se vuelve a leer la carpeta: la devuelta cae en su sitio.
        self.capturas =
            crate::galeria_capturas::listar(&crate::galeria_capturas::carpeta(&self.ubicacion))
                .into_iter()
                .take(disposicion::MINIATURAS)
                .collect();
        if !self.minis.contains_key(&b.original) {
            let mini = reducida(&b.original);
            let m = match mini {
                Some(imagen) => Mini::Lista {
                    bitmap: None,
                    imagen,
                },
                None => Mini::Imposible,
            };
            self.minis.insert(b.original.clone(), m);
        }
        let t = self.textos.t("bandeja2-devuelta");
        self.avisar(t);
    }
}

/// Lo que pide una pulsacion al bucle del hilo.
enum Hacer {
    Nada,
    /// Mandar la accion al hilo principal.
    Pedir(Accion),
    Cerrar,
}

/// Lo que hace un clic en `zona`.
fn al_pulsar(panel: &mut Panel, zona: Zona) -> Hacer {
    match zona {
        Zona::Sincronizar => Hacer::Pedir(Accion::Comando(Comando::Sincronizar)),
        Zona::Ajustes => Hacer::Pedir(Accion::Comando(Comando::AbrirAjustes)),
        Zona::Buscador => Hacer::Nada,
        Zona::Capturar => Hacer::Pedir(Accion::Comando(Comando::CapturarRegion)),
        Zona::Modo(i) => Hacer::Pedir(Accion::Comando(MODOS[i].0)),
        Zona::EditarFavoritos | Zona::Anadir => {
            panel.editando = true;
            panel.consulta.clear();
            panel.elegida = 0;
            panel.scroll = 0.0;
            Hacer::Nada
        }
        Zona::Favorito(i) => panel
            .favoritos
            .get(i)
            .map_or(Hacer::Nada, |a| Hacer::Pedir(*a)),
        Zona::Interruptor(i) => Hacer::Pedir(Accion::Comando(INTERRUPTORES[i])),
        Zona::VerGaleria => Hacer::Pedir(Accion::Galeria),
        Zona::Miniatura(i) => match panel.capturas.get(i) {
            Some(c) => {
                if let Err(e) = pixpin_shell::abrir(&c.ruta) {
                    panel.fallo(e.to_string());
                    Hacer::Nada
                } else {
                    Hacer::Cerrar
                }
            }
            None => Hacer::Nada,
        },
        Zona::AccionMiniatura(i, a) => match a {
            AccionMiniatura::Pinear => {
                let Some(ruta) = panel.capturas.get(i).map(|c| c.ruta.clone()) else {
                    return Hacer::Nada;
                };
                // Por el mismo camino que «Abrir con PixPin»: la ventana
                // principal, que es la que tiene los pines, la pinea.
                if pixpin_shell::mensajero::enviar_ficheros(&[ruta]) {
                    Hacer::Cerrar
                } else {
                    panel.fallo("PixPin".into());
                    Hacer::Nada
                }
            }
            AccionMiniatura::Copiar => {
                panel.copiar(i);
                Hacer::Nada
            }
            AccionMiniatura::Borrar => {
                panel.borrar(i);
                Hacer::Nada
            }
        },
        Zona::Ventana(i) => Hacer::Pedir(VENTANAS[i]),
        Zona::Salir => Hacer::Pedir(Accion::Comando(Comando::Salir)),
        Zona::Fila(i) => activar_fila(panel, i),
        Zona::Hecho => {
            panel.editando = false;
            panel.consulta.clear();
            panel.scroll = 0.0;
            Hacer::Nada
        }
    }
}

/// Intro o clic en una fila: hacerla, o al elegir favoritos, marcarla.
fn activar_fila(panel: &mut Panel, i: usize) -> Hacer {
    let filas = panel.filas();
    let Some(a) = filas.get(i).copied() else {
        return Hacer::Nada;
    };
    panel.elegida = i;
    if panel.editando {
        if acciones::alternar_favorito(&mut panel.favoritos, a) {
            panel.guardar_favoritos();
        } else {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("tope", pixpin_store::bandeja::TOPE);
            let t = panel.textos.t_args("bandeja2-favoritos-llenos", &args);
            panel.avisar(t);
        }
        return Hacer::Nada;
    }
    Hacer::Pedir(a)
}

/// Una tecla con el panel delante. Devuelve lo que hay que hacer despues.
fn al_teclear(panel: &mut Panel, vk: u32, ctrl: bool) -> Hacer {
    const ATRAS: u32 = 0x08;
    const INTRO: u32 = 0x0D;
    const ESC: u32 = 0x1B;
    const ARRIBA: u32 = 0x26;
    const ABAJO: u32 = 0x28;
    const SUPR: u32 = 0x2E;
    let en_lista = panel.editando || !panel.consulta.trim().is_empty();
    match vk {
        ESC => {
            if !panel.consulta.is_empty() {
                panel.consulta.clear();
                panel.elegida = 0;
            } else if panel.editando {
                panel.editando = false;
            } else {
                return Hacer::Cerrar;
            }
            panel.scroll = 0.0;
            Hacer::Nada
        }
        ATRAS => {
            panel.consulta.pop();
            panel.elegida = 0;
            panel.scroll = 0.0;
            Hacer::Nada
        }
        INTRO if en_lista => {
            let i = panel.elegida;
            activar_fila(panel, i)
        }
        INTRO => Hacer::Pedir(Accion::Comando(Comando::CapturarRegion)),
        ARRIBA | ABAJO if en_lista => {
            let n = panel.filas().len();
            if n > 0 {
                panel.elegida = if vk == ABAJO {
                    (panel.elegida + 1).min(n - 1)
                } else {
                    panel.elegida.saturating_sub(1)
                };
                asomar_elegida(panel);
            }
            Hacer::Nada
        }
        0x5A if ctrl => {
            panel.deshacer();
            Hacer::Nada
        }
        0x43 if ctrl && !en_lista => {
            let d = panel.disposicion();
            if let Some(i) = panel.miniatura_viva(&d) {
                panel.copiar(i);
            }
            Hacer::Nada
        }
        SUPR if !en_lista => {
            let d = panel.disposicion();
            if let Some(i) = panel.miniatura_viva(&d) {
                panel.borrar(i);
            }
            Hacer::Nada
        }
        _ => Hacer::Nada,
    }
}

/// Desplaza lo justo para que la fila elegida se vea.
fn asomar_elegida(panel: &mut Panel) {
    let d = panel.disposicion();
    let Some(f) = d.filas.get(panel.elegida) else {
        return;
    };
    let alto = panel.marco.alto as f32;
    let arriba = d.buscador.y + d.buscador.alto;
    if f.y < arriba {
        panel.scroll -= arriba - f.y;
    } else if f.y + f.alto > alto {
        panel.scroll += f.y + f.alto - alto;
    }
    panel.scroll = panel.scroll.clamp(0.0, panel.scroll_maximo());
}

/// Un caracter escrito va al buscador.
fn al_escribir(panel: &mut Panel, c: char) {
    if c.is_control() || (c == ' ' && panel.consulta.is_empty()) {
        return;
    }
    panel.consulta.push(c);
    panel.elegida = 0;
    panel.scroll = 0.0;
}

// ------------------------------------------------------------- el hilo

fn vivir() -> Result<()> {
    // Si la GPU se pierde con el panel abierto, se cierra (WM_CLOSE); y con
    // el cerrado, el hilo se va en cuanto llega un pedido (mas abajo), que
    // lo vuelve a lanzar con un dispositivo nuevo.
    pixpin_render::perdida::cerrar_ventanas_al_perder(true);
    let recursos = Recursos::nuevos()?;
    let motor = recursos.motor();
    let inicial = Rect {
        x: -10_000,
        y: -10_000,
        ancho: 440,
        alto: 800,
    };
    let mut ventana = VentanaOverlay::nueva(inicial).context("sin ventana para el panel")?;
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        inicial.ancho,
        inicial.alto,
    )
    .context("sin superficie para el panel")?;
    let hwnd = ventana.handle().0 as isize;
    VENTANA.store(hwnd, Ordering::SeqCst);
    let mut panel: Option<Panel> = None;
    let mut cerrado_desde = Instant::now();

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let eventos = pixpin_shell::overlay::tomar_eventos_pendientes();

        let Some(p) = panel.as_mut() else {
            // Cerrado con el dispositivo perdido: el hilo se va sin tocar el
            // pedido, y al irse se relanza (ver `lanzar`) con uno nuevo.
            if let Some(motivo) = recursos.perdido() {
                tracing::warn!(
                    motivo = %crate::dispositivo_perdido::describir(motivo),
                    "dispositivo grafico perdido; el panel de la bandeja renace"
                );
                break;
            }
            // Cerrado: esperar un pedido, o irse si tarda.
            if let Some(pedido) = tomar_pedido() {
                match Panel::nuevo(pedido, hwnd) {
                    Ok(nuevo) => {
                        ventana.mover(nuevo.marco);
                        let _ = superficie.redimensionar(nuevo.marco.ancho, nuevo.marco.alto);
                        let mut nuevo = nuevo;
                        pintar_y_presentar(&mut nuevo, &superficie, &motor);
                        ventana.mostrar();
                        ventana.traer_encima();
                        ventana.enfocar();
                        VISIBLE.store(true, Ordering::SeqCst);
                        CERRAR.store(false, Ordering::SeqCst);
                        tracing::info!(marco = ?nuevo.marco, "panel de la bandeja abierto");
                        panel = Some(nuevo);
                    }
                    Err(e) => tracing::warn!(?e, "no se pudo preparar el panel de la bandeja"),
                }
                continue;
            }
            if cerrado_desde.elapsed() > VIVE_CERRADO {
                break;
            }
            pixpin_shell::overlay::esperar_eventos(Some(1_000));
            continue;
        };

        let mut pintar = false;
        let mut hacer = Hacer::Nada;
        for (h, ev) in eventos {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |q: Punto, m: Rect| ((q.x - m.x) as f32, (q.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => hacer = Hacer::Cerrar,
                EventoOverlay::RatonMovido(q) => p.raton = local(q, p.marco),
                EventoOverlay::BotonPulsado(q) => {
                    p.raton = local(q, p.marco);
                    if let Some(b) = p.boton_deshacer
                        && disposicion::dentro(b, p.raton)
                    {
                        p.deshacer();
                        continue;
                    }
                    let d = p.disposicion();
                    if let Some(z) = d.zona_en(p.raton, p.miniatura_viva(&d)) {
                        hacer = al_pulsar(p, z);
                    }
                }
                EventoOverlay::Rueda(m) => {
                    let paso = disposicion::FILA * p.escala;
                    p.scroll =
                        (p.scroll - m as f32 / 120.0 * paso * 1.5).clamp(0.0, p.scroll_maximo());
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => hacer = al_teclear(p, vk, ctrl),
                EventoOverlay::Caracter(c) => al_escribir(p, c),
                _ => {}
            }
            if !matches!(hacer, Hacer::Nada) {
                break;
            }
        }

        // Otro clic en el icono con el panel abierto lo cierra.
        if CERRAR.swap(false, Ordering::SeqCst) {
            hacer = Hacer::Cerrar;
        }
        // Perder el foco lo cierra, como el centro de control de Windows.
        let delante = pixpin_render::superficie::en_primer_plano(ventana.handle());
        if delante {
            p.fue_delante = true;
        } else if p.fue_delante && matches!(hacer, Hacer::Nada) {
            CERRADO_POR_FOCO_MS.store(pixpin_shell::entorno::ahora_utc_ms(), Ordering::SeqCst);
            hacer = Hacer::Cerrar;
        }

        match hacer {
            Hacer::Nada => {}
            Hacer::Cerrar => {
                cerrar(&ventana, &mut panel, &mut cerrado_desde);
                continue;
            }
            Hacer::Pedir(a) => {
                tracing::info!(?a, "panel de la bandeja: accion");
                if a.cierra_el_panel() {
                    cerrar(&ventana, &mut panel, &mut cerrado_desde);
                    if a.mira_la_pantalla() {
                        // Que el panel no salga en la captura.
                        pixpin_shell::overlay::esperar_composicion();
                    }
                }
                if !pixpin_shell::mensajero::pedir_ventana_principal(a.id()) {
                    tracing::warn!(?a, "la ventana principal no recogio la accion del panel");
                }
                if panel.is_none() {
                    continue;
                }
            }
        }
        let Some(p) = panel.as_mut() else {
            continue;
        };

        if p.atender_llegadas() {
            pintar = true;
        }
        if p.aviso
            .as_ref()
            .is_some_and(|a| a.desde.elapsed() > Duration::from_millis(5_000))
        {
            p.aviso = None;
            pintar = true;
        }
        // Lo que publico el hilo principal (un interruptor) llega con un toque.
        if pintar || p.aviso.is_some() {
            pintar_y_presentar(p, &superficie, &motor);
        }
        pixpin_shell::overlay::esperar_eventos(Some(if p.aviso.is_some() { 250 } else { 150 }));
    }
    VISIBLE.store(false, Ordering::SeqCst);
    tracing::debug!("hilo del panel de la bandeja terminado");
    Ok(())
}

fn cerrar(ventana: &VentanaOverlay, panel: &mut Option<Panel>, desde: &mut Instant) {
    ventana.ocultar();
    VISIBLE.store(false, Ordering::SeqCst);
    *panel = None;
    *desde = Instant::now();
}

fn pintar_y_presentar(p: &mut Panel, superficie: &Superficie, motor: &MotorRender) {
    p.scroll = p.scroll.clamp(0.0, p.scroll_maximo());
    p.subir(motor);
    if let Ok(destino) = superficie.empezar(motor) {
        let _ = motor.dibujar(&destino, |pintor: &Pintor| pintar(p, pintor));
        let _ = superficie.presentar();
    }
}

// ------------------------------------------------------------- pintar

fn negrita() -> Letra<'static> {
    Letra {
        negrita: true,
        ..Letra::de(pixpin_render::letras::LETRA_DEL_SISTEMA)
    }
}

fn icono_en(
    p: &Pintor,
    icono: &pixpin_render::icono::Icono,
    cx: f32,
    cy: f32,
    lado: f32,
    color: Color,
) {
    p.icono(
        icono,
        RectF {
            x: cx - lado / 2.0,
            y: cy - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        color,
    );
}

/// Un texto con su centro vertical en `cy`.
fn texto_en(p: &Pintor, t: &str, x: f32, cy: f32, tam: f32, ancho: f32, color: Color) {
    let (_, h) = p.medir_texto(t, tam);
    p.texto_linea(t, x, cy - h / 2.0, tam, ancho.max(0.0), color);
}

fn texto_negrita_en(p: &Pintor, t: &str, x: f32, cy: f32, tam: f32, color: Color) {
    let l = negrita();
    let (_, h) = p.medir_con_letra(t, tam, SIN_PARTIR, &l);
    p.texto_con_letra(t, x, cy - h / 2.0, tam, SIN_PARTIR, &l, color);
}

/// La chapita de un atajo, pegada a la derecha en `x_der`. Devuelve su ancho.
fn chapita(p: &Pintor, t: &str, x_der: f32, cy: f32, e: f32, fondo: Color, color: Color) -> f32 {
    let tam = 11.0 * e;
    let (w, h) = p.medir_texto(t, tam);
    let caja = RectF {
        x: x_der - w - 12.0 * e,
        y: cy - (h + 8.0 * e) / 2.0,
        ancho: w + 12.0 * e,
        alto: h + 8.0 * e,
    };
    p.rellenar_redondeado(caja, 6.0 * e, fondo);
    p.texto(t, caja.x + 6.0 * e, caja.y + 4.0 * e, tam, color);
    caja.ancho
}

fn interruptor(p: &Pintor, x_der: f32, cy: f32, e: f32, encendido: bool, pal: &Paleta) {
    let caja = RectF {
        x: x_der - 40.0 * e,
        y: cy - 12.0 * e,
        ancho: 40.0 * e,
        alto: 24.0 * e,
    };
    p.rellenar_redondeado(
        caja,
        12.0 * e,
        if encendido { pal.verde } else { pal.apagado },
    );
    let cx = if encendido {
        caja.x + 28.0 * e
    } else {
        caja.x + 12.0 * e
    };
    p.circulo((cx, cy), 10.0 * e, Color::BLANCO);
}

fn insignia(p: &Pintor, t: &str, x_der: f32, cy: f32, e: f32, fondo: Color, color: Color) {
    let tam = 11.0 * e;
    let l = negrita();
    let (w, h) = p.medir_con_letra(t, tam, SIN_PARTIR, &l);
    let ancho = (w + 12.0 * e).max(20.0 * e);
    let caja = RectF {
        x: x_der - ancho,
        y: cy - 10.0 * e,
        ancho,
        alto: 20.0 * e,
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    p.texto_con_letra(
        t,
        caja.x + (ancho - w) / 2.0,
        cy - h / 2.0,
        tam,
        SIN_PARTIR,
        &l,
        color,
    );
}

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}

fn pintar(panel: &mut Panel, p: &Pintor) {
    let pal = panel.paleta;
    let e = panel.escala;
    let (w, h) = (panel.marco.ancho as f32, panel.marco.alto as f32);
    let d = panel.disposicion();
    let viva = panel.miniatura_viva(&d);
    let encima = d.zona_en(panel.raton, viva);
    let textos = &panel.textos;

    p.limpiar_transparente();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    p.rellenar_redondeado(todo, 18.0 * e, pal.borde);
    p.rellenar_redondeado(encoger(todo, e.max(1.0)), 17.0 * e, pal.fondo);

    // --- cabecera
    p.rellenar_redondeado(d.logo, 8.0 * e, pal.azul);
    let (lw, _) = p.medir_con_letra("P", 14.0 * e, SIN_PARTIR, &negrita());
    texto_negrita_en(
        p,
        "P",
        d.logo.x + (d.logo.ancho - lw) / 2.0,
        d.logo.y + d.logo.alto / 2.0,
        14.0 * e,
        Color::BLANCO,
    );
    let xt = d.logo.x + d.logo.ancho + 8.0 * e;
    let ancho_t = d.sincronizar.x - 8.0 * e - xt;
    let cy = d.logo.y + d.logo.alto / 2.0;
    texto_negrita_en(
        p,
        &textos.t("app-nombre"),
        xt,
        cy - 8.0 * e,
        15.0 * e,
        pal.texto,
    );
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let (punto, estado) = if panel.ultima_sincro > 0 {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuando", hace_cuanto(textos, panel.ultima_sincro, ahora));
        (pal.verde, textos.t_args("bandeja2-sincronizado", &args))
    } else {
        (pal.suave, textos.t("bandeja2-sin-sincronizar"))
    };
    p.circulo((xt + 3.5 * e, cy + 10.0 * e), 3.5 * e, punto);
    texto_en(
        p,
        &estado,
        xt + 13.0 * e,
        cy + 10.0 * e,
        12.0 * e,
        ancho_t - 13.0 * e,
        pal.suave,
    );

    let s = d.sincronizar;
    let fondo_s = if encima == Some(Zona::Sincronizar) {
        alfa(pal.texto, 0.16)
    } else {
        alfa(pal.texto, 0.08)
    };
    p.rellenar_redondeado(s, 9.0 * e, fondo_s);
    icono_en(
        p,
        &iconos::SINCRONIZAR,
        s.x + 17.0 * e,
        s.y + s.alto / 2.0,
        14.0 * e,
        pal.texto,
    );
    texto_en(
        p,
        &textos.t("bandeja2-sincronizar"),
        s.x + 30.0 * e,
        s.y + s.alto / 2.0,
        12.0 * e,
        s.ancho - 36.0 * e,
        pal.texto,
    );
    if encima == Some(Zona::Ajustes) {
        p.rellenar_redondeado(d.ajustes, 10.0 * e, pal.velo);
    }
    icono_en(
        p,
        &iconos::AJUSTES,
        d.ajustes.x + d.ajustes.ancho / 2.0,
        d.ajustes.y + d.ajustes.alto / 2.0,
        20.0 * e,
        pal.suave2,
    );

    // --- buscador
    let b = d.buscador;
    p.rellenar_redondeado(b, 12.0 * e, pal.azul_claro);
    p.rellenar_redondeado(encoger(b, e.max(1.0)), 11.0 * e, pal.carta);
    let cyb = b.y + b.alto / 2.0;
    icono_en(p, &iconos::BUSCAR, b.x + 22.0 * e, cyb, 20.0 * e, pal.suave);
    let xb = b.x + 42.0 * e;
    if panel.consulta.is_empty() {
        let pista = if panel.editando {
            textos.t("bandeja2-buscar-favorito")
        } else {
            textos.t("bandeja2-buscar")
        };
        texto_en(p, &pista, xb, cyb, 14.0 * e, b.ancho - 60.0 * e, pal.suave);
        // El cursor, para que se vea que basta con escribir.
        p.rellenar(
            RectF {
                x: xb - 2.0 * e,
                y: cyb - 9.0 * e,
                ancho: (1.5 * e).max(1.0),
                alto: 18.0 * e,
            },
            pal.azul_claro,
        );
    } else {
        let ancho_chapa = 40.0 * e;
        texto_en(
            p,
            &panel.consulta,
            xb,
            cyb,
            14.0 * e,
            b.ancho - 60.0 * e - ancho_chapa,
            pal.texto,
        );
        let (tw, _) = p.medir_texto(&panel.consulta, 14.0 * e);
        let xc = (xb + tw + 1.0 * e).min(b.x + b.ancho - 12.0 * e - ancho_chapa);
        p.rellenar(
            RectF {
                x: xc,
                y: cyb - 9.0 * e,
                ancho: (1.5 * e).max(1.0),
                alto: 18.0 * e,
            },
            pal.azul_claro,
        );
        chapita(
            p,
            "Esc",
            b.x + b.ancho - 12.0 * e,
            cyb,
            e,
            pal.chapa,
            pal.suave2,
        );
    }

    // Lo de debajo del buscador se recorta: al desplazar no pisa la cabecera.
    let corte = RectF {
        x: 0.0,
        y: b.y + b.alto + 2.0 * e,
        ancho: w,
        alto: (h - (b.y + b.alto + 2.0 * e)).max(0.0),
    };
    let modo = panel.modo();
    p.con_recorte(corte, |p| match modo {
        Modo::Normal => pintar_normal(panel, p, &d, encima, viva),
        Modo::Busqueda { .. } | Modo::Editar { .. } => pintar_lista(panel, p, &d, encima),
    });

    // --- el aviso, encima de todo
    panel.boton_deshacer = None;
    if let Some(a) = &panel.aviso {
        let tam = 14.0 * e;
        let (tw, th) = p.medir_texto(&a.texto, tam);
        let rotulo = textos.t("bandeja2-deshacer");
        let (dw, _) = p.medir_texto(&rotulo, tam);
        let extra = if a.deshacer.is_some() {
            dw + 70.0 * e
        } else {
            0.0
        };
        let ancho = (tw + 28.0 * e + extra).min(w - 28.0 * e);
        let alto = (th + 16.0 * e).max(44.0 * e);
        let caja = RectF {
            x: (w - ancho) / 2.0,
            y: h - alto - 14.0 * e,
            ancho,
            alto,
        };
        p.rellenar_redondeado(caja, 10.0 * e, alfa(Color::NEGRO, 0.92));
        texto_en(
            p,
            &a.texto,
            caja.x + 14.0 * e,
            caja.y + alto / 2.0,
            tam,
            ancho - 28.0 * e - extra,
            Color::BLANCO,
        );
        if a.deshacer.is_some() {
            let boton = RectF {
                x: caja.x + ancho - extra - 4.0 * e,
                y: caja.y + 2.0 * e,
                ancho: extra,
                alto: alto - 4.0 * e,
            };
            if disposicion::dentro(boton, panel.raton) {
                p.rellenar_redondeado(boton, 8.0 * e, alfa(Color::BLANCO, 0.12));
            }
            texto_negrita_en(
                p,
                &rotulo,
                boton.x + 8.0 * e,
                boton.y + boton.alto / 2.0,
                tam,
                hex(0x64D2FF),
            );
            chapita(
                p,
                "Ctrl Z",
                boton.x + boton.ancho - 8.0 * e,
                boton.y + boton.alto / 2.0,
                e,
                alfa(Color::BLANCO, 0.14),
                hex(0xC7C7CC),
            );
            panel.boton_deshacer = Some(boton);
        }
    }
}

fn etiqueta(p: &Pintor, t: &str, x: f32, cy: f32, e: f32, color: Color) {
    let l = negrita();
    let t = t.to_uppercase();
    let (_, h) = p.medir_con_letra(&t, 11.0 * e, SIN_PARTIR, &l);
    p.texto_con_letra(&t, x, cy - h / 2.0, 11.0 * e, SIN_PARTIR, &l, color);
}

fn pintar_normal(
    panel: &Panel,
    p: &Pintor,
    d: &Disposicion,
    encima: Option<Zona>,
    viva: Option<usize>,
) {
    let pal = panel.paleta;
    let e = panel.escala;
    let textos = &panel.textos;

    // --- Capturar
    p.rellenar_redondeado(d.tarjeta_captura, 14.0 * e, pal.carta);
    let c = d.capturar;
    p.rellenar_redondeado(
        c,
        12.0 * e,
        if encima == Some(Zona::Capturar) {
            pal.azul_encima
        } else {
            pal.azul
        },
    );
    let cy = c.y + c.alto / 2.0;
    icono_en(
        p,
        &iconos::CAPTURAR,
        c.x + 26.0 * e,
        cy,
        24.0 * e,
        Color::BLANCO,
    );
    let x = c.x + 50.0 * e;
    let mut derecha = c.x + c.ancho - 14.0 * e;
    if let Some(a) = panel.atajos.get(&Comando::CapturarRegion.id()) {
        derecha -= chapita(
            p,
            a,
            derecha,
            cy,
            e,
            alfa(Color::BLANCO, 0.2),
            Color::BLANCO,
        ) + 8.0 * e;
    }
    texto_negrita_en(
        p,
        &textos.t("bandeja2-capturar"),
        x,
        cy - 9.0 * e,
        16.0 * e,
        Color::BLANCO,
    );
    texto_en(
        p,
        &textos.t("bandeja2-capturar-sub"),
        x,
        cy + 10.0 * e,
        12.0 * e,
        derecha - x,
        alfa(Color::BLANCO, 0.9),
    );
    for (i, m) in d.modos.iter().enumerate() {
        let (comando, clave) = MODOS[i];
        let fondo = if encima == Some(Zona::Modo(i)) {
            alfa(pal.texto, 0.12)
        } else {
            alfa(pal.texto, 0.06)
        };
        p.rellenar_redondeado(*m, 10.0 * e, fondo);
        let cx = m.x + m.ancho / 2.0;
        let cyi = m.y + 17.0 * e;
        icono_en(
            p,
            Accion::Comando(comando).icono(),
            cx,
            cyi,
            20.0 * e,
            pal.texto,
        );
        if comando == Comando::GrabarGif {
            p.circulo((cx, cyi), 3.0 * e, pal.rojo);
        }
        let t = textos.t(clave);
        let tam = 11.0 * e;
        let (tw, _) = p.medir_texto(&t, tam);
        p.texto_linea(
            &t,
            cx - (tw.min(m.ancho - 4.0 * e)) / 2.0,
            m.y + 30.0 * e,
            tam,
            m.ancho - 4.0 * e,
            pal.texto,
        );
    }

    // --- Favoritos
    let ef = d.etiqueta_favoritos;
    etiqueta(
        p,
        &textos.t("bandeja2-favoritos"),
        ef.x + 4.0 * e,
        ef.y + ef.alto / 2.0,
        e,
        pal.suave,
    );
    let ed = d.editar;
    if encima == Some(Zona::EditarFavoritos) {
        p.rellenar_redondeado(ed, 8.0 * e, pal.velo);
    }
    let rotulo = textos.t("bandeja2-editar");
    let (rw, _) = p.medir_texto(&rotulo, 12.0 * e);
    let xr = ed.x + ed.ancho - 8.0 * e - rw;
    icono_en(
        p,
        &iconos::LAPIZ,
        xr - 10.0 * e,
        ed.y + ed.alto / 2.0,
        14.0 * e,
        pal.enlace,
    );
    texto_en(
        p,
        &rotulo,
        xr,
        ed.y + ed.alto / 2.0,
        12.0 * e,
        rw + 2.0 * e,
        pal.enlace,
    );
    for (i, f) in d.favoritos.iter().enumerate() {
        let a = panel.favoritos[i];
        p.rellenar_redondeado(
            *f,
            12.0 * e,
            if encima == Some(Zona::Favorito(i)) {
                pal.carta_encima
            } else {
                pal.carta
            },
        );
        let cx = f.x + f.ancho / 2.0;
        let punto = RectF {
            x: cx - 14.0 * e,
            y: f.y + 8.0 * e,
            ancho: 28.0 * e,
            alto: 28.0 * e,
        };
        p.rellenar_redondeado(punto, 9.0 * e, a.color());
        icono_en(
            p,
            a.icono(),
            cx,
            punto.y + 14.0 * e,
            20.0 * e,
            Color::BLANCO,
        );
        let t = panel.titulo(a);
        let tam = 12.0 * e;
        let (tw, _) = p.medir_texto(&t, tam);
        let ancho = f.ancho - 8.0 * e;
        p.texto_linea(
            &t,
            cx - tw.min(ancho) / 2.0,
            f.y + 40.0 * e,
            tam,
            ancho,
            pal.texto,
        );
    }
    if let Some(a) = d.anadir {
        if encima == Some(Zona::Anadir) {
            p.rellenar_redondeado(a, 12.0 * e, pal.velo);
        }
        p.trazar_discontinuo(encoger(a, e), 1.5 * e, alfa(pal.texto, 0.25));
        let cx = a.x + a.ancho / 2.0;
        icono_en(p, &iconos::MAS, cx, a.y + 22.0 * e, 20.0 * e, pal.suave);
        let t = textos.t("bandeja2-anadir");
        let (tw, _) = p.medir_texto(&t, 12.0 * e);
        p.texto_linea(
            &t,
            cx - tw / 2.0,
            a.y + 40.0 * e,
            12.0 * e,
            a.ancho,
            pal.suave,
        );
    }

    // --- Interruptores
    let est = estado();
    p.rellenar_redondeado(d.tarjeta_interruptores, 14.0 * e, pal.carta);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", est.a_la_vista);
    let ep = d.etiqueta_pines;
    etiqueta(
        p,
        &textos.t_args("bandeja2-pines", &args),
        ep.x,
        ep.y + ep.alto / 2.0 + 2.0 * e,
        e,
        pal.suave,
    );
    let ea = d.etiqueta_atajos;
    etiqueta(
        p,
        &textos.t("bandeja2-atajos"),
        ea.x,
        ea.y + ea.alto / 2.0,
        e,
        pal.suave,
    );
    p.rellenar(d.raya, pal.borde);
    let filas = [
        (
            &iconos::OJO_TACHADO,
            "bandeja2-ocultar-pines",
            est.ocultos,
            None,
        ),
        (&iconos::CURSOR, "bandeja2-paso-clic", est.pasantes, None),
        (
            &iconos::TECLADO,
            "bandeja2-silenciar",
            est.silenciados,
            Some("bandeja2-silenciar-sub"),
        ),
    ];
    for (i, f) in d.interruptores.iter().enumerate() {
        let (icono, clave, encendido, sub) = filas[i];
        if encima == Some(Zona::Interruptor(i)) {
            p.rellenar_redondeado(*f, 10.0 * e, pal.velo);
        }
        let cy = f.y + f.alto / 2.0;
        icono_en(p, icono, f.x + 22.0 * e, cy, 20.0 * e, pal.suave);
        let x = f.x + 44.0 * e;
        let mut derecha = f.x + f.ancho - 12.0 * e;
        interruptor(p, derecha, cy, e, encendido, &pal);
        derecha -= 40.0 * e + 10.0 * e;
        if let Some(a) = panel.atajos.get(&INTERRUPTORES[i].id()) {
            derecha -= chapita(p, a, derecha, cy, e, pal.chapa, pal.suave2) + 8.0 * e;
        }
        if let Some(sub) = sub {
            let t = textos.t(sub);
            let (sw, _) = p.medir_texto(&t, 12.0 * e);
            let sw = sw.min((derecha - x) * 0.5);
            texto_en(p, &t, derecha - sw, cy, 12.0 * e, sw, pal.suave);
            derecha -= sw + 8.0 * e;
        }
        texto_en(p, &textos.t(clave), x, cy, 14.0 * e, derecha - x, pal.texto);
    }

    // --- Ultimas capturas
    let eu = d.etiqueta_ultimas;
    etiqueta(
        p,
        &textos.t("bandeja2-ultimas"),
        eu.x + 4.0 * e,
        eu.y + eu.alto / 2.0,
        e,
        pal.suave,
    );
    let vg = d.ver_galeria;
    let t = textos.t("bandeja2-ver-galeria");
    let (tw, _) = p.medir_texto(&t, 12.0 * e);
    if encima == Some(Zona::VerGaleria) {
        p.rellenar_redondeado(vg, 8.0 * e, pal.velo);
    }
    texto_en(
        p,
        &t,
        vg.x + vg.ancho - 4.0 * e - tw,
        vg.y + vg.alto / 2.0,
        12.0 * e,
        tw + 2.0 * e,
        pal.enlace,
    );
    if panel.capturas.is_empty() {
        let (a, z) = (d.miniaturas[0], d.miniaturas[d.miniaturas.len() - 1]);
        let caja = RectF {
            x: a.x,
            y: a.y,
            ancho: z.x + z.ancho - a.x,
            alto: a.alto,
        };
        p.rellenar_redondeado(caja, 10.0 * e, alfa(pal.texto, 0.04));
        let t = textos.t("bandeja2-sin-capturas");
        let (tw, _) = p.medir_texto(&t, 13.0 * e);
        texto_en(
            p,
            &t,
            caja.x + (caja.ancho - tw) / 2.0,
            caja.y + caja.alto / 2.0,
            13.0 * e,
            caja.ancho,
            pal.suave,
        );
    }
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    for (i, m) in d.miniaturas.iter().enumerate() {
        let Some(captura) = panel.capturas.get(i) else {
            if !panel.capturas.is_empty() {
                p.rellenar_redondeado(*m, 10.0 * e, alfa(pal.texto, 0.04));
            }
            continue;
        };
        p.rellenar_redondeado(*m, 10.0 * e, pal.carta);
        match panel.minis.get(&captura.ruta) {
            Some(Mini::Lista {
                bitmap: Some(bm),
                imagen,
            }) => {
                p.con_recorte(*m, |p| {
                    crate::miniaturas::pintar_recortado(p, bm, *m, imagen.ancho, imagen.alto);
                });
            }
            Some(Mini::Imposible) | Some(Mini::Lista { bitmap: None, .. }) => {
                let icono = if es_video(&captura.ruta) {
                    &iconos::GRABAR
                } else {
                    &iconos::GALERIA
                };
                icono_en(
                    p,
                    icono,
                    m.x + m.ancho / 2.0,
                    m.y + m.alto / 2.0,
                    22.0 * e,
                    pal.suave,
                );
            }
            _ => {}
        }
        // Cuando: «10:02» si es de hoy, si no la fecha corta.
        let ms = crate::caducidad_capturas::ms_de(captura.cuando);
        let dia = |x: i64| pixpin_shell::entorno::a_local(x).div_euclid(86_400_000);
        let cuando = if dia(ms) == dia(ahora) {
            let min = pixpin_shell::entorno::a_local(ms).rem_euclid(86_400_000) / 60_000;
            format!("{:02}:{:02}", min / 60, min % 60)
        } else if dia(ms) == dia(ahora) - 1 {
            textos.t("bandeja2-ayer")
        } else {
            crate::ventana_chat::fecha_corta(textos, ms)
        };
        let tam = 10.0 * e;
        let (cw, ch) = p.medir_texto(&cuando, tam);
        let pastilla = RectF {
            x: m.x + 4.0 * e,
            y: m.y + m.alto - ch - 8.0 * e,
            ancho: (cw + 8.0 * e).min(m.ancho - 8.0 * e),
            alto: ch + 4.0 * e,
        };
        p.rellenar_redondeado(pastilla, 4.0 * e, alfa(Color::NEGRO, 0.55));
        p.texto_linea(
            &cuando,
            pastilla.x + 4.0 * e,
            pastilla.y + 2.0 * e,
            tam,
            pastilla.ancho - 6.0 * e,
            Color::BLANCO,
        );
        if viva == Some(i) {
            p.rellenar_redondeado(*m, 10.0 * e, alfa(Color::NEGRO, 0.35));
            p.trazar(encoger(*m, e), 2.0 * e, pal.azul_claro);
            let botones = disposicion::acciones_de_miniatura(*m, e);
            let iconos_b = [
                (&iconos::PIN, Color::BLANCO),
                (&iconos::COPIAR, Color::BLANCO),
                (&iconos::BORRAR, hex(0xFF6961)),
            ];
            for (j, bt) in botones.iter().enumerate() {
                let sobre =
                    encima == Some(Zona::AccionMiniatura(i, disposicion::ACCIONES_MINIATURA[j]));
                p.rellenar_redondeado(
                    *bt,
                    8.0 * e,
                    alfa(Color::NEGRO, if sobre { 0.8 } else { 0.55 }),
                );
                let (ic, col) = iconos_b[j];
                icono_en(
                    p,
                    ic,
                    bt.x + bt.ancho / 2.0,
                    bt.y + bt.alto / 2.0,
                    16.0 * e,
                    col,
                );
            }
        }
    }

    // --- Ventanas
    for (i, v) in d.ventanas.iter().enumerate() {
        let a = VENTANAS[i];
        p.rellenar_redondeado(
            *v,
            12.0 * e,
            if encima == Some(Zona::Ventana(i)) {
                pal.carta_encima
            } else {
                pal.carta
            },
        );
        let cy = v.y + v.alto / 2.0;
        let color = match a {
            Accion::Tareas => pal.verde,
            Accion::Lecciones => pal.amarillo,
            Accion::Comando(Comando::AbrirChat) => pal.enlace,
            _ => pal.suave2,
        };
        icono_en(p, a.icono(), v.x + 22.0 * e, cy, 20.0 * e, color);
        let mut derecha = v.x + v.ancho - 10.0 * e;
        match a {
            Accion::Tareas if panel.tareas.unwrap_or(0) > 0 => {
                let n = panel.tareas.unwrap_or(0).to_string();
                insignia(p, &n, derecha, cy, e, pal.azul, Color::BLANCO);
                derecha -= 30.0 * e;
            }
            Accion::Lecciones if panel.lecciones.unwrap_or(0) > 0 => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("n", panel.lecciones.unwrap_or(0));
                let t = textos.t_args("bandeja2-para-hoy", &args);
                insignia(
                    p,
                    &t,
                    derecha,
                    cy,
                    e,
                    alfa(hex(0xFF9F0A), 0.18),
                    pal.naranja,
                );
                let (tw, _) = p.medir_texto(&t, 11.0 * e);
                derecha -= tw + 20.0 * e;
            }
            _ => {}
        }
        let x = v.x + 40.0 * e;
        texto_en(p, &panel.titulo(a), x, cy, 14.0 * e, derecha - x, pal.texto);
    }

    // --- Salir
    p.rellenar(d.raya_salir, pal.borde);
    let s = d.salir;
    if encima == Some(Zona::Salir) {
        p.rellenar_redondeado(s, 10.0 * e, alfa(pal.rojo, 0.12));
    }
    let cy = s.y + s.alto / 2.0;
    icono_en(p, &iconos::SALIR, s.x + 22.0 * e, cy, 20.0 * e, pal.rojo);
    let sub = textos.t("bandeja2-salir-sub");
    let (sw, _) = p.medir_texto(&sub, 12.0 * e);
    texto_en(
        p,
        &sub,
        s.x + s.ancho - 12.0 * e - sw,
        cy,
        12.0 * e,
        sw + 2.0 * e,
        pal.suave,
    );
    texto_en(
        p,
        &textos.t("bandeja2-salir"),
        s.x + 44.0 * e,
        cy,
        14.0 * e,
        s.ancho - 68.0 * e - sw,
        pal.rojo,
    );
}

fn pintar_lista(panel: &Panel, p: &Pintor, d: &Disposicion, encima: Option<Zona>) {
    let pal = panel.paleta;
    let e = panel.escala;
    let textos = &panel.textos;
    let filas = panel.filas();

    if let (Some(cab), Some(hecho)) = (d.cabecera_lista, d.hecho) {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("tope", pixpin_store::bandeja::TOPE);
        args.set("n", panel.favoritos.len());
        let t = textos.t_args("bandeja2-elige-favoritos", &args);
        texto_en(
            p,
            &t,
            cab.x + 4.0 * e,
            cab.y + cab.alto / 2.0,
            13.0 * e,
            hecho.x - cab.x - 12.0 * e,
            pal.suave,
        );
        p.rellenar_redondeado(
            hecho,
            10.0 * e,
            if encima == Some(Zona::Hecho) {
                pal.azul_encima
            } else {
                pal.azul
            },
        );
        let t = textos.t("bandeja2-hecho");
        let l = negrita();
        let (tw, _) = p.medir_con_letra(&t, 14.0 * e, SIN_PARTIR, &l);
        texto_negrita_en(
            p,
            &t,
            hecho.x + (hecho.ancho - tw) / 2.0,
            hecho.y + hecho.alto / 2.0,
            14.0 * e,
            Color::BLANCO,
        );
    }

    if filas.is_empty() && !panel.consulta.trim().is_empty() {
        let t = textos.t("bandeja2-sin-resultados");
        let y = d.buscador.y + d.buscador.alto + 30.0 * e;
        let (tw, _) = p.medir_texto(&t, 14.0 * e);
        texto_en(p, &t, (d.ancho - tw) / 2.0, y, 14.0 * e, d.ancho, pal.suave);
        return;
    }

    for (i, f) in d.filas.iter().enumerate() {
        let Some(a) = filas.get(i).copied() else {
            continue;
        };
        if i == panel.elegida {
            p.rellenar_redondeado(*f, 10.0 * e, alfa(pal.azul_claro, 0.22));
        } else if encima == Some(Zona::Fila(i)) {
            p.rellenar_redondeado(*f, 10.0 * e, pal.velo);
        }
        let cy = f.y + f.alto / 2.0;
        let mut derecha = f.x + f.ancho - 12.0 * e;
        if panel.editando {
            let punto = RectF {
                x: f.x + 8.0 * e,
                y: cy - 14.0 * e,
                ancho: 28.0 * e,
                alto: 28.0 * e,
            };
            p.rellenar_redondeado(punto, 9.0 * e, a.color());
            icono_en(p, a.icono(), cy_x(punto), cy, 18.0 * e, Color::BLANCO);
            let elegido = panel.favoritos.contains(&a);
            let centro = (derecha - 11.0 * e, cy);
            if elegido {
                p.circulo(centro, 11.0 * e, pal.azul_claro);
                icono_en(p, &iconos::VISTO, centro.0, cy, 14.0 * e, Color::BLANCO);
            } else {
                p.anillo(centro, 10.0 * e, 1.5 * e, alfa(pal.texto, 0.35));
            }
            derecha -= 30.0 * e;
        } else {
            icono_en(p, a.icono(), f.x + 22.0 * e, cy, 20.0 * e, pal.suave2);
            if let Accion::Comando(c) = a
                && let Some(t) = panel.atajos.get(&c.id())
            {
                derecha -= chapita(p, t, derecha, cy, e, pal.chapa, pal.suave2) + 8.0 * e;
            }
            if i == panel.elegida {
                derecha -= chapita(p, "Intro", derecha, cy, e, pal.chapa, pal.suave2) + 8.0 * e;
            }
        }
        let x = f.x + 44.0 * e;
        texto_en(p, &panel.titulo(a), x, cy, 14.0 * e, derecha - x, pal.texto);
    }
}

fn cy_x(r: RectF) -> f32 {
    r.x + r.ancho / 2.0
}

// ------------------------------------------------------------- pruebas

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_ultima_sincro_es_la_mas_reciente_de_los_aparatos() {
        let dir = std::env::temp_dir().join(format!("pixpin-panel-sincro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(ultima_sincro(&dir), 0, "caso negativo: sin carpeta, nunca");
        let elegidos = dir.join("sincro").join("elegidos");
        std::fs::create_dir_all(&elegidos).unwrap();
        std::fs::write(elegidos.join("a.cuando"), "1000\n").unwrap();
        std::fs::write(elegidos.join("b.cuando"), "5000").unwrap();
        std::fs::write(elegidos.join("c.cuando"), "roto").unwrap();
        std::fs::write(elegidos.join("d.txt"), "9999").unwrap();
        assert_eq!(ultima_sincro(&dir), 5000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publicar_solo_cambia_si_cambia() {
        let a = Interruptores {
            a_la_vista: 3,
            ocultos: false,
            pasantes: true,
            silenciados: false,
        };
        publicar(a);
        assert_eq!(estado(), a);
        publicar(a);
        assert_eq!(estado(), a);
    }

    fn panel_de_prueba(textos: Catalogo) -> Panel {
        let (_tx, rx) = channel();
        let titulos = acciones::todas()
            .into_iter()
            .map(|a| (a, textos.t(a.clave_titulo())))
            .collect();
        Panel {
            textos,
            ubicacion: Ubicacion::Portable {
                raiz: std::env::temp_dir().join("pixpin-panel-no-se-usa"),
            },
            atajos: [
                (Comando::CapturarRegion.id(), "Ctrl Alt X".to_string()),
                (Comando::AlternarPines.id(), "Ctrl 2".to_string()),
            ]
            .into_iter()
            .collect(),
            paleta: Paleta::oscura(),
            escala: 1.0,
            marco: Rect {
                x: 0,
                y: 0,
                ancho: 440,
                alto: 820,
            },
            consulta: String::new(),
            editando: false,
            elegida: 0,
            scroll: 0.0,
            favoritos: acciones::favoritos_de(
                &pixpin_store::bandeja::DE_FABRICA
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            ),
            titulos,
            capturas: Vec::new(),
            minis: HashMap::new(),
            tareas: Some(4),
            lecciones: Some(2),
            ultima_sincro: 0,
            raton: (-1.0, -1.0),
            aviso: None,
            boton_deshacer: None,
            llegadas: rx,
            fue_delante: false,
        }
    }

    #[test]
    fn escribir_busca_y_esc_deshace_por_pasos() {
        let mut p = panel_de_prueba(Catalogo::nuevo(Idioma::Espanol));
        // Un espacio al principio no abre una busqueda vacia.
        al_escribir(&mut p, ' ');
        assert_eq!(p.modo(), Modo::Normal);
        for c in "tarea".chars() {
            al_escribir(&mut p, c);
        }
        assert!(matches!(p.modo(), Modo::Busqueda { resultados } if resultados >= 1));
        assert!(p.filas().contains(&Accion::Tareas));
        // Caso negativo: un caracter de control (Ctrl+Z llega asi) no se escribe.
        al_escribir(&mut p, '\u{1a}');
        assert_eq!(p.consulta, "tarea");
        // Atras borra una letra.
        assert!(matches!(al_teclear(&mut p, 0x08, false), Hacer::Nada));
        assert_eq!(p.consulta, "tare");
        // Esc: primero borra lo buscado, luego sale de editar, y al final cierra.
        p.editando = true;
        assert!(matches!(al_teclear(&mut p, 0x1B, false), Hacer::Nada));
        assert!(p.consulta.is_empty() && p.editando);
        assert!(matches!(al_teclear(&mut p, 0x1B, false), Hacer::Nada));
        assert!(!p.editando);
        assert!(matches!(al_teclear(&mut p, 0x1B, false), Hacer::Cerrar));
    }

    #[test]
    fn sin_buscar_intro_captura_y_las_flechas_no_salen_de_la_lista() {
        let mut p = panel_de_prueba(Catalogo::nuevo(Idioma::Espanol));
        assert!(matches!(
            al_teclear(&mut p, 0x0D, false),
            Hacer::Pedir(Accion::Comando(Comando::CapturarRegion))
        ));
        for c in "capt".chars() {
            al_escribir(&mut p, c);
        }
        let n = p.filas().len();
        assert!(n >= 2);
        for _ in 0..n + 5 {
            al_teclear(&mut p, 0x28, false);
        }
        assert_eq!(
            p.elegida,
            n - 1,
            "caso negativo: la flecha no se pasa del final"
        );
        for _ in 0..n + 5 {
            al_teclear(&mut p, 0x26, false);
        }
        assert_eq!(p.elegida, 0);
        // Ctrl+Z sin nada que deshacer no hace nada.
        assert!(matches!(al_teclear(&mut p, 0x5A, true), Hacer::Nada));
    }

    #[test]
    fn al_editar_un_clic_marca_y_desmarca_favoritos() {
        let mut p = panel_de_prueba(Catalogo::nuevo(Idioma::Espanol));
        p.ubicacion = Ubicacion::Portable {
            raiz: std::env::temp_dir().join(format!("pixpin-panel-fav-{}", std::process::id())),
        };
        assert!(matches!(
            al_pulsar(&mut p, Zona::EditarFavoritos),
            Hacer::Nada
        ));
        assert!(p.editando);
        let antes = p.favoritos.len();
        let primera = p.filas()[0];
        let estaba = p.favoritos.contains(&primera);
        assert!(
            matches!(activar_fila(&mut p, 0), Hacer::Nada),
            "al editar no se hace, se marca"
        );
        assert_eq!(p.favoritos.contains(&primera), !estaba);
        assert_ne!(p.favoritos.len(), antes);
        // Y quedo guardado en los ajustes.
        let guardados = pixpin_store::bandeja::leer(&p.ubicacion);
        assert_eq!(guardados.len(), p.favoritos.len());
        let _ = std::fs::remove_dir_all(p.ubicacion.raiz());
    }

    #[test]
    fn intro_hace_la_fila_elegida_y_al_editar_la_marca() {
        let mut p = panel_de_prueba(Catalogo::nuevo(Idioma::Espanol));
        for c in "galer".chars() {
            al_escribir(&mut p, c);
        }
        assert!(matches!(
            activar_fila(&mut p, 0),
            Hacer::Pedir(Accion::Galeria)
        ));
        assert!(
            matches!(activar_fila(&mut p, 99), Hacer::Nada),
            "caso negativo: fila que no hay"
        );
    }

    #[test]
    fn el_panel_entero_cabe_en_la_maqueta() {
        let p = panel_de_prueba(Catalogo::nuevo(Idioma::Espanol));
        let d = p.disposicion_normal();
        assert!(d.alto_total <= 820.0, "{}", d.alto_total);
    }

    /// **Lo que se ve**, pintado de verdad en un PNG (necesita GPU). Con
    /// `PIXPIN_MUESTRAS` apuntando a una carpeta, deja ahi las capturas.
    #[test]
    #[ignore = "pinta con la GPU; para mirar el panel"]
    fn muestra_del_panel() {
        let textos = || Catalogo::nuevo(Idioma::Espanol);
        let mut p = panel_de_prueba(textos());
        p.ultima_sincro = pixpin_shell::entorno::ahora_utc_ms() - 2 * 60_000;
        publicar(Interruptores {
            a_la_vista: 7,
            ocultos: false,
            pasantes: true,
            silenciados: false,
        });
        let d = p.disposicion_normal();
        let alto = d.alto_total.round() as u32;
        p.marco.alto = alto;
        p.raton = (d.favoritos[0].x + 10.0, d.favoritos[0].y + 10.0);
        crate::ventanita::muestra("bandeja2-normal", 440, alto, |pintor, _| {
            pintar(&mut p, pintor)
        });

        let mut p = panel_de_prueba(textos());
        p.marco.alto = alto;
        p.consulta = "cap".into();
        crate::ventanita::muestra("bandeja2-buscar", 440, alto, |pintor, _| {
            pintar(&mut p, pintor)
        });

        let mut p = panel_de_prueba(textos());
        p.marco.alto = alto;
        p.editando = true;
        p.aviso = Some(Aviso {
            texto: "Borrada".into(),
            deshacer: Some(Borrada {
                original: PathBuf::new(),
                en_papelera: PathBuf::new(),
                conservada: false,
            }),
            desde: Instant::now(),
        });
        crate::ventanita::muestra("bandeja2-editar", 440, alto, |pintor, _| {
            pintar(&mut p, pintor)
        });

        let mut p = panel_de_prueba(textos());
        p.paleta = Paleta::clara();
        p.marco.alto = alto;
        crate::ventanita::muestra("bandeja2-claro", 440, alto, |pintor, _| {
            pintar(&mut p, pintor)
        });
    }
}
