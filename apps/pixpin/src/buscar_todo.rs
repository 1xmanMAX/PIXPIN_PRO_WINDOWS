//! **Buscar en PixPin** (rediseno v2, maquetas `Buscar2-vacio` y
//! `Buscar2-resultados`): una ventana que busca en todo —archivos y
//! mensajes de los chats, tareas, lecciones, capturas— y hace con lo
//! elegido lo mismo que el plugin de Flow Launcher (`p`).
//!
//! **No hay buscador nuevo**: los resultados salen de `pixpin_lanzador`
//! (`resultados::resultados`, `capturas::lista`, `menu::menu`), el mismo
//! codigo que contesta a Flow. Asi «t comprar pan», «g muro» o «lecciones
//! encofrado» dan lo mismo en los dos sitios, y una mejora del plugin llega
//! sola aqui. Lo que hace cada resultado es un pedido del protocolo
//! (`docs/protocolo-pedidos.md`) que se le manda a la propia app, como si
//! viniera de Flow: lo atiende `pedidos.rs` en el bucle principal.
//!
//! Con la caja vacia ensena las busquedas recientes, lo abierto hace poco y
//! las letras rapidas (t, n, l, g, c, u, a: las mismas que en Flow). Con
//! algo escrito, pestanas con su cuenta, la fila elegida con sus botones y
//! la vista previa. Todo se hace con el teclado: flechas, Tab (o Ctrl+Tab)
//! entre pestanas, Intro, Ctrl+Intro, Ctrl+C, flecha derecha (mas
//! acciones), Alt+Intro (abrir con), Supr (borrar una captura, con
//! deshacer) y Esc.
//!
//! Se abre desde la bandeja («Buscar en PixPin») o con el atajo que el
//! usuario le ponga: nace sin atajo, como casi todo (el usuario no quiere
//! mas atajos de fabrica).

#![forbid(unsafe_code)]

mod disposicion;
mod modelo;
mod pintar;
mod recientes;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_lanzador::consulta::{Funcion, Modo, analizar};
use pixpin_lanzador::resultados::{self, Accion, Contexto, Resultado};
use pixpin_lanzador::{capturas, datos::Datos, imagenes};
use pixpin_render::{Pintor, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use serde_json::{Value, json};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::overlay::Recursos;
use disposicion::Zona;
use modelo::{Estado, Orden, Pestana};
use pintar::{Detalle, LetraRapida, Paleta, Vista};
use recientes::Recientes;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Cuantos «abiertos hace poco» se ensenan.
const ABIERTOS_A_LA_VISTA: usize = 5;
/// Cuantas capturas se mezclan en «Todo» al buscar.
const CAPTURAS_EN_TODO: usize = 6;
/// Lado mayor de las fotos que se cargan (miniatura y vista previa).
const LADO_FOTO: u32 = 480;
/// Cuantas fotos se tienen cargadas a la vez.
const TOPE_FOTOS: usize = 48;

/// Lo que el buscador necesita saber de la app al abrirse.
#[derive(Debug, Clone, Default)]
pub struct Opciones {
    /// El atajo del buscador, si el usuario le puso uno («Ctrl Espacio»).
    pub atajo: Option<String>,
    /// El atajo de capturar una zona, para la letra «c».
    pub atajo_capturar: Option<String>,
    /// La ventana de mensajes de la app (`HWND` como numero), donde se
    /// mandan los pedidos. 0: se busca por su clase.
    pub mensajes: isize,
}

impl Opciones {
    /// Las de los ajustes de ahora.
    pub fn de(config: &pixpin_store::Ajustes, mensajes: isize) -> Opciones {
        use pixpin_store::comandos::{Comando, Enlaces};
        let (enlaces, _) = Enlaces::de_ajustes(config);
        let texto = |c: Comando| enlaces.atajo_de(c).map(|a| a.to_string().replace('+', " "));
        Opciones {
            atajo: texto(Comando::Buscar),
            atajo_capturar: texto(Comando::CapturarRegion),
            mensajes,
        }
    }
}

/// **Abre el buscador**, o lo trae delante si ya estaba. Vuelve enseguida:
/// la ventana vive en su propio hilo.
pub fn abrir(idioma: Idioma, ubicacion: Ubicacion, opciones: Opciones) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new().name("buscar-todo".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let textos = Catalogo::nuevo(idioma);
        let hecho = Recursos::nuevos().and_then(|r| bucle(&r, &textos, &ubicacion, &opciones));
        if let Err(e) = hecho {
            tracing::warn!(?e, "no se pudo abrir el buscador");
        }
        ABIERTA.store(0, Ordering::SeqCst);
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del buscador");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// ------------------------------------------------------------ las fotos

/// Una foto pedida al hilo que las lee: la reducida y lo que mide la
/// original.
struct FotoLeida {
    ruta: String,
    imagen: Option<pixpin_codec::ImagenRgba>,
    original: (u32, u32),
}

/// El hilo que lee y reduce las fotos: leer una captura de pantalla entera
/// son decenas de milisegundos, y la ventana no puede pararse a eso.
fn lanzar_lector(hwnd: isize) -> (Sender<String>, Receiver<FotoLeida>) {
    let (pedir, pedidas) = channel::<String>();
    let (dar, dadas) = channel::<FotoLeida>();
    let _ = std::thread::Builder::new().name("buscar-fotos".into()).spawn(move || {
        for ruta in pedidas {
            let leida = pixpin_codec::imagen::cargar(Path::new(&ruta)).ok();
            let original = leida.as_ref().map_or((0, 0), |i| (i.ancho, i.alto));
            let imagen = leida.and_then(|i| {
                let lado = i.ancho.max(i.alto);
                if lado <= LADO_FOTO {
                    return Some(i);
                }
                let k = LADO_FOTO as f32 / lado as f32;
                let (w, h) = (((i.ancho as f32 * k) as u32).max(1), ((i.alto as f32 * k) as u32).max(1));
                pixpin_codec::imagen::redimensionar(i, w, h).ok()
            });
            if dar.send(FotoLeida { ruta, imagen, original }).is_err() {
                break;
            }
            pixpin_shell::overlay::despertar(hwnd);
        }
    });
    (pedir, dadas)
}

#[derive(Default)]
struct Fotos {
    listas: HashMap<String, (ID2D1Bitmap1, u32, u32)>,
    originales: HashMap<String, (u32, u32)>,
    pedidas: HashSet<String>,
    orden: Vec<String>,
}

impl Fotos {
    fn pedir(&mut self, ruta: &str, pedir: &Sender<String>) {
        if self.listas.contains_key(ruta) || self.pedidas.contains(ruta) {
            return;
        }
        self.pedidas.insert(ruta.to_string());
        let _ = pedir.send(ruta.to_string());
    }

    fn recibir(&mut self, f: FotoLeida, motor: &pixpin_render::MotorRender) {
        self.originales.insert(f.ruta.clone(), f.original);
        let Some(i) = f.imagen else { return };
        if let Ok(b) = motor.bitmap_desde_pixeles_premultiplicado(i.ancho, i.alto, &i.pixeles) {
            self.listas.insert(f.ruta.clone(), (b, i.ancho, i.alto));
            self.orden.push(f.ruta);
        }
        while self.orden.len() > TOPE_FOTOS {
            let vieja = self.orden.remove(0);
            self.listas.remove(&vieja);
            self.pedidas.remove(&vieja);
        }
    }
}

// --------------------------------------------------------- lo que se busca

/// Lo que hace falta para preguntar al plugin.
struct Fuente {
    raiz: PathBuf,
    datos: Datos,
}

impl Fuente {
    fn contexto(&self) -> Contexto {
        let mut ctx = Contexto::con("", pixpin_lanzador::datos::ahora_ms());
        ctx.raiz = Some(self.raiz.clone());
        ctx
    }
}

/// **Los resultados de lo escrito**: los del plugin, y al buscar en todo
/// (sin verbo ni letra delante) tambien las capturas que coincidan, que el
/// plugin solo da con «capturas …».
fn resultados_de(consulta: &str, pestana: Pestana, fuente: &mut Fuente, recientes: &Recientes) -> Vec<Resultado> {
    let ctx = fuente.contexto();
    let proyectos = fuente.datos.proyectos();
    if consulta.trim().is_empty() {
        return match pestana.consulta_vacia() {
            None => abiertos(recientes, &proyectos, &ctx),
            Some(c) => resultados::resultados(&proyectos, c, &ctx),
        };
    }
    let mut v = resultados::resultados(&proyectos, consulta, &ctx);
    if matches!(analizar(consulta), Modo::Buscar { proyecto: None, .. }) {
        let ya: HashSet<Option<String>> = v.iter().map(|r| r.clave.clone()).collect();
        v.extend(
            capturas::lista(&ctx, consulta.trim())
                .into_iter()
                .filter(|r| modelo::pestana_de(r) == Pestana::Capturas && !ya.contains(&r.clave))
                .take(CAPTURAS_EN_TODO),
        );
    }
    v
}

/// «Abierto hace poco»: lo elegido aqui, y si es poco, la ultima captura y
/// los ultimos proyectos (lo que tambien ensena Flow sin escribir nada).
fn abiertos(recientes: &Recientes, proyectos: &[pixpin_lanzador::datos::Proyecto], ctx: &Contexto) -> Vec<Resultado> {
    let mut v: Vec<Resultado> = recientes.abiertos.iter().filter_map(|a| a.resultado()).collect();
    if v.len() < ABIERTOS_A_LA_VISTA {
        if let Some(raiz) = ctx.raiz.as_deref() {
            if let Some(c) = capturas::leer(raiz, ctx.ahora).first() {
                v.push(capturas::resultado(c, ctx));
            }
        }
        v.extend(
            resultados::resultados(proyectos, "", ctx)
                .into_iter()
                .filter(|r| modelo::pestana_de(r) == Pestana::Archivos),
        );
    }
    let mut vistas = HashSet::new();
    v.retain(|r| vistas.insert(r.clave.clone().unwrap_or_else(|| r.titulo.clone())));
    v.truncate(ABIERTOS_A_LA_VISTA);
    v
}

/// Las letras rapidas, en el orden de la maqueta, con lo que hacen en el
/// plugin (`consulta::Funcion::atajo`).
fn letras(textos: &Catalogo, raiz: &Path, opciones: &Opciones) -> Vec<LetraRapida> {
    let ahora = pixpin_lanzador::datos::ahora_ms();
    ['t', 'n', 'l', 'g', 'c', 'u', 'a']
        .into_iter()
        .filter(|l| Funcion::atajo(&l.to_string()).is_some())
        .map(|l| {
            let sub = match l {
                'u' => capturas::leer(raiz, ahora)
                    .first()
                    .map(|c| capturas::subtitulo(c, ahora))
                    .unwrap_or_else(|| textos.t("buscar-todo-letra-u-sub-nada")),
                _ => textos.t(&format!("buscar-todo-letra-{l}-sub")),
            };
            LetraRapida {
                letra: l,
                titulo: textos.t(&format!("buscar-todo-letra-{l}")),
                sub,
                atajo: if l == 'c' { opciones.atajo_capturar.clone() } else { None },
            }
        })
        .collect()
}

// --------------------------------------------------------------- hacer

/// Lo que queda despues de hacer una accion.
#[derive(Debug, PartialEq)]
enum Tras {
    Cerrar,
    /// Se queda abierto; volver a buscar dentro de un rato (lo que cambio lo
    /// escribe la app en su bucle).
    Refrescar,
    /// Se queda abierto y lo escrito cambio.
    Buscar,
    Nada,
}

/// Una captura que se llevo a la papelera, para deshacer.
struct Borrada {
    original: PathBuf,
    en_papelera: PathBuf,
    conservada: bool,
}

struct Ventana<'a> {
    textos: &'a Catalogo,
    fuente: Fuente,
    opciones: &'a Opciones,
    estado: Estado,
    recientes: Recientes,
    aviso: Option<(String, Instant)>,
    borrada: Option<Borrada>,
    refrescar: Option<Instant>,
}

impl Ventana<'_> {
    fn raiz(&self) -> PathBuf {
        self.fuente.raiz.clone()
    }

    fn buscar(&mut self, mantener: bool) {
        let antes = self.estado.elegido;
        let v = resultados_de(&self.estado.consulta, self.estado.pestana, &mut self.fuente, &self.recientes);
        self.estado.poner_resultados(v);
        if mantener {
            self.estado.elegido = antes.min(self.estado.visibles().len().saturating_sub(1));
        }
    }

    fn avisar(&mut self, clave: &str) {
        self.aviso = Some((self.textos.t(clave), Instant::now()));
    }

    /// Manda un pedido a la app (lo atiende `pedidos.rs` en el bucle
    /// principal, como uno de Flow).
    fn pedir(&self, p: &Value) {
        let json = p.to_string();
        let r = if self.opciones.mensajes != 0 {
            pixpin_shell::mensajero::enviar_pedido_a(windows::Win32::Foundation::HWND(self.opciones.mensajes as *mut _), &json)
        } else {
            pixpin_shell::mensajero::enviar_pedido(&json)
        };
        tracing::debug!(respuesta = r, %json, "pedido del buscador");
    }

    fn hacer(&mut self, accion: Accion) -> Tras {
        match accion {
            Accion::Pedido(p) => match p.get("accion").and_then(Value::as_str).unwrap_or("") {
                // Borrar desde el menu: el de aqui, que se puede deshacer.
                "borrar_captura" => match p.get("ruta").and_then(Value::as_str) {
                    Some(r) => self.borrar_captura(r),
                    None => Tras::Nada,
                },
                "copiar_imagen" => {
                    self.pedir(&p);
                    self.avisar("buscar-todo-copiado");
                    Tras::Nada
                }
                "conservar_captura" => {
                    self.pedir(&p);
                    self.avisar("buscar-todo-conservada-ya");
                    Tras::Refrescar
                }
                _ => {
                    self.pedir(&p);
                    Tras::Cerrar
                }
            },
            Accion::PedirYSeguir { pedido, consulta } => {
                self.pedir(&pedido);
                self.estado.poner_consulta(&consulta);
                Tras::Refrescar
            }
            Accion::Consulta(c) => {
                self.estado.poner_consulta(&c);
                Tras::Buscar
            }
            Accion::Copiar(t) => {
                match pixpin_codec::copiar_texto(&t) {
                    Ok(()) => self.avisar("buscar-todo-copiado"),
                    Err(e) => tracing::warn!(?e, "no se pudo copiar el texto"),
                }
                Tras::Nada
            }
            Accion::Carpeta { carpeta, fichero } => {
                let hecho = if fichero.is_empty() {
                    pixpin_shell::abrir::abrir(Path::new(&carpeta))
                } else {
                    pixpin_shell::abrir::abrir_ubicacion(Path::new(&fichero))
                };
                if let Err(e) = hecho {
                    tracing::warn!(?e, "no se pudo abrir la carpeta");
                }
                Tras::Cerrar
            }
            Accion::Windows(r) => {
                if let Err(e) = pixpin_shell::abrir::abrir(Path::new(&r)) {
                    tracing::warn!(?e, "no se pudo abrir con Windows");
                }
                Tras::Cerrar
            }
            Accion::PegarImagen { consulta, numero } => {
                let ahora = pixpin_lanzador::datos::ahora_ms();
                let _ = imagenes::pegar_en_borrador(&self.raiz(), &imagenes::WINDOWS, Some(numero), ahora);
                self.estado.poner_consulta(&consulta);
                Tras::Buscar
            }
        }
    }

    /// Ctrl+V: una imagen va como la ficha `[img NN]` (la tarea o la leccion
    /// que se escribe la lleva), un texto se escribe.
    fn pegar(&mut self) -> Tras {
        let pp = imagenes::WINDOWS;
        if (pp.imagen)().is_some() {
            let numero = imagenes::siguiente(&self.estado.consulta);
            let ahora = pixpin_lanzador::datos::ahora_ms();
            let n = match imagenes::pegar_en_borrador(&self.raiz(), &pp, Some(numero), ahora) {
                imagenes::Pegado::Nueva(n, _) | imagenes::Pegado::Repetida(n) => n,
                imagenes::Pegado::Nada => return Tras::Nada,
            };
            let mut c = self.estado.consulta.trim_end().to_string();
            if !c.is_empty() {
                c.push(' ');
            }
            c.push_str(&imagenes::ficha(n));
            c.push(' ');
            self.estado.poner_consulta(&c);
            return Tras::Buscar;
        }
        if let Some(pixpin_codec::ContenidoPortapapeles::Texto(t)) = pixpin_codec::leer() {
            let linea = t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
            if linea.is_empty() {
                return Tras::Nada;
            }
            let c = format!("{}{}", self.estado.consulta, linea);
            self.estado.poner_consulta(&c);
            return Tras::Buscar;
        }
        Tras::Nada
    }

    /// Supr: la captura a la papelera de PixPin (como el boton de la
    /// galeria), con deshacer.
    fn borrar_captura(&mut self, ruta: &str) -> Tras {
        let raiz = self.raiz();
        let ruta = PathBuf::from(ruta);
        // Solo las de la carpeta de capturas: la ruta viene de un resultado.
        if ruta.parent() != Some(capturas::carpeta(&raiz).as_path()) {
            return Tras::Nada;
        }
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let nombre = crate::caducidad_capturas::nombre(&ruta);
        let conservada = crate::caducidad_capturas::leer(&raiz, ahora).conservadas.contains(&nombre);
        match crate::galeria_capturas::a_la_papelera(&raiz, &ruta) {
            Ok(en_papelera) => {
                if conservada {
                    let _ = crate::caducidad_capturas::cambiar(&raiz, ahora, |r| {
                        r.conservadas.remove(&nombre);
                    });
                }
                tracing::info!(ruta = %ruta.display(), "captura a la papelera desde el buscador");
                self.borrada = Some(Borrada { original: ruta, en_papelera, conservada });
                self.estado.se_puede_deshacer = true;
                self.avisar("buscar-todo-borrada");
                Tras::Buscar
            }
            Err(e) => {
                tracing::warn!(?e, "no se pudo borrar la captura");
                Tras::Nada
            }
        }
    }

    fn deshacer(&mut self) -> Tras {
        let Some(b) = self.borrada.take() else { return Tras::Nada };
        self.estado.se_puede_deshacer = false;
        self.aviso = None;
        if b.original.exists() || std::fs::rename(&b.en_papelera, &b.original).is_err() {
            tracing::warn!(ruta = %b.original.display(), "no se pudo devolver la captura");
            return Tras::Nada;
        }
        if b.conservada {
            let nombre = crate::caducidad_capturas::nombre(&b.original);
            let _ = crate::caducidad_capturas::cambiar(&self.raiz(), pixpin_shell::entorno::ahora_utc_ms(), |r| {
                r.conservadas.insert(nombre);
            });
        }
        Tras::Buscar
    }

    /// El menu del plugin para el elegido (lo de la flecha derecha).
    fn opciones_de_menu(&self) -> Vec<Resultado> {
        let Some(c) = self.estado.elegido().and_then(|r| r.contexto.clone()) else {
            return Vec::new();
        };
        pixpin_lanzador::menu::menu(&c, &self.fuente.contexto())
    }

    /// **Atiende una orden** del modelo.
    fn orden(&mut self, o: Orden, hwnd: windows::Win32::Foundation::HWND) -> Tras {
        match o {
            Orden::Nada | Orden::Repintar => Tras::Nada,
            Orden::Buscar => Tras::Buscar,
            Orden::Cerrar => Tras::Cerrar,
            Orden::Hacer { accion, recordar } => {
                if recordar {
                    if let Some(r) = self.estado.elegido().cloned() {
                        self.recientes.apuntar_abierto(&r);
                    }
                    let c = self.estado.consulta.clone();
                    self.recientes.apuntar_busqueda(&c);
                    self.recientes.guardar(&self.raiz());
                }
                self.hacer(accion)
            }
            Orden::Pegar => self.pegar(),
            Orden::BorrarCaptura(r) => self.borrar_captura(&r),
            Orden::Deshacer => self.deshacer(),
            Orden::AbrirCon(f) => match pixpin_shell::abrir_con_otra::abrir_con_otra(hwnd, Path::new(&f)) {
                Ok(true) => Tras::Cerrar,
                Ok(false) => Tras::Nada,
                Err(e) => {
                    tracing::warn!(?e, "no se pudo abrir «Abrir con»");
                    Tras::Nada
                }
            },
        }
    }

    /// Lo que sabe la vista previa del elegido (las capturas: medidas, peso
    /// y cuando caducan).
    fn detalle(&self, fotos: &Fotos) -> Detalle {
        let Some(r) = self.estado.elegido() else { return Detalle::default() };
        let Some(ruta) = modelo::ruta_de_captura(r) else { return Detalle::default() };
        let mut d = Detalle::default();
        let peso = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
        if let Some((w, h)) = fotos.originales.get(&ruta).filter(|(w, _)| *w > 0) {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("ancho", *w);
            args.set("alto", *h);
            args.set("peso", peso_legible(peso));
            d.medidas = Some(self.textos.t_args("buscar-todo-medidas", &args));
        }
        let ahora = pixpin_lanzador::datos::ahora_ms();
        if let Some(c) = capturas::leer(&self.fuente.raiz, ahora).into_iter().find(|c| c.ruta.to_string_lossy() == ruta) {
            d.caducidad = Some(match c.se_va {
                None => (self.textos.t("buscar-todo-conservada"), false),
                Some(t) => {
                    let dias = ((t - ahora).max(0) + 86_399_999) / 86_400_000;
                    let texto = if dias <= 0 {
                        self.textos.t("buscar-todo-caduca-hoy")
                    } else {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("dias", dias);
                        self.textos.t_args("buscar-todo-caduca", &args)
                    };
                    (texto, true)
                }
            });
        }
        d
    }
}

/// «412 KB», «1,2 MB».
fn peso_legible(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

// ---------------------------------------------------------------- bucle

fn bucle(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion, opciones: &Opciones) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = crate::ventanita::centrado(monitor.area_trabajo, disposicion::ANCHO, disposicion::ALTO, monitor.escala_por_cien);
    // Algo por encima del centro, como los buscadores del sistema.
    let area = monitor.area_trabajo;
    marco.y = area.y + ((area.alto - marco.alto) as i32 / 4).max(0);
    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("buscar-todo-titulo")).context("no se pudo abrir el buscador")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), marco.ancho, marco.alto)
        .context("sin superficie para el buscador")?;
    ventana.mostrar();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    ABIERTA.store(hwnd, Ordering::SeqCst);

    let raiz = ubicacion.raiz().to_path_buf();
    let pal = if pixpin_shell::entorno::tema_claro() && !crate::tema_cosmos::activo() { Paleta::DIA } else { Paleta::NOCHE };
    let letras_rapidas = letras(textos, &raiz, opciones);
    let mut v = Ventana {
        textos,
        // Con la cache de disco del plugin: abrir no relee todos los
        // proyectos si Flow ya los leyo.
        fuente: Fuente { raiz: raiz.clone(), datos: Datos::nuevo(raiz.clone()).con_cache_en_disco() },
        opciones,
        estado: Estado::default(),
        recientes: Recientes::leer(&raiz),
        aviso: None,
        borrada: None,
        refrescar: None,
    };
    v.buscar(false);
    let (pedir_foto, fotos_leidas) = lanzar_lector(hwnd);
    let mut fotos = Fotos::default();
    let mut raton = (-1.0f32, -1.0f32);
    let mut scroll = 0.0f32;
    let mut dispo = disposicion::Disposicion::default();
    let mut vivo = true;
    let mut pintar = true;
    tracing::info!("buscador abierto");
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut tras = Vec::new();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(p) => {
                    raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    tras.push(clic(&mut v, &dispo, raton, ventana.handle()));
                }
                EventoOverlay::Rueda(m) => {
                    let paso = if m > 0 { modelo::VK_UP } else { modelo::VK_DOWN };
                    for _ in 0..3 {
                        v.estado.tecla(paso, false, false, false);
                    }
                }
                EventoOverlay::Tecla { vk, shift, ctrl, alt } => {
                    // La flecha derecha abre el menu del plugin (lo sabe la
                    // ventana, no el modelo).
                    let o = if vk == modelo::VK_RIGHT && v.estado.menu.is_none() && !v.estado.es_inicio() {
                        let opciones = v.opciones_de_menu();
                        v.estado.abrir_menu(opciones)
                    } else {
                        v.estado.tecla(vk, shift, ctrl, alt)
                    };
                    tras.push(v.orden(o, ventana.handle()));
                }
                EventoOverlay::Caracter(c) => {
                    // Lo que ya atendio la tecla con Ctrl (Ctrl+C, Ctrl+V…)
                    // llega tambien como caracter de control: se ignora.
                    let o = v.estado.caracter(c);
                    tras.push(v.orden(o, ventana.handle()));
                }
                _ => {}
            }
        }
        for t in tras {
            match t {
                Tras::Cerrar => vivo = false,
                Tras::Buscar => {
                    v.buscar(false);
                    scroll = 0.0;
                }
                Tras::Refrescar => {
                    v.buscar(true);
                    v.refrescar = Some(Instant::now() + Duration::from_millis(400));
                }
                Tras::Nada => {}
            }
        }
        if !vivo {
            break;
        }
        if v.refrescar.is_some_and(|t| Instant::now() >= t) {
            v.refrescar = None;
            v.buscar(true);
            pintar = true;
        }
        if v.aviso.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(if v.borrada.is_some() { 6_000 } else { 2_000 })) {
            v.aviso = None;
            v.borrada = None;
            v.estado.se_puede_deshacer = false;
            pintar = true;
        }
        while let Ok(f) = fotos_leidas.try_recv() {
            fotos.recibir(f, &motor);
            pintar = true;
        }

        if pintar {
            // Las fotos que hacen falta: la de cada fila visible y la del
            // elegido.
            for r in v.estado.visibles().iter().take(30).map(|i| &v.estado.resultados[*i]) {
                if let Some(f) = r.vista_previa.as_deref().or(r.icono.as_deref()) {
                    fotos.pedir(f, &pedir_foto);
                }
            }
            let hay_menu = !v.estado.es_inicio() && !v.opciones_de_menu().is_empty();
            let detalle = v.detalle(&fotos);
            let aviso = v.aviso.as_ref().map(|(t, _)| t.clone());
            let vista = Vista {
                estado: &v.estado,
                busquedas: &v.recientes.busquedas,
                letras: &letras_rapidas,
                atajo: opciones.atajo.as_deref(),
                fotos: &fotos.listas,
                detalle,
                hay_menu,
                aviso: aviso.as_deref(),
                raton,
                scroll,
            };
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    dispo = pintar::pintar(p, &vista, &pal, textos, marco.ancho as f32, marco.alto as f32, e);
                });
                let _ = superficie.presentar();
            }
            scroll = dispo.scroll;
            pintar = false;
        }
        let espera = if v.aviso.is_some() || v.refrescar.is_some() { 200 } else { 1_000 };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    v.fuente.datos.guardar_cache();
    tracing::info!("buscador cerrado");
    Ok(())
}

/// Un clic: lo que hay debajo segun la disposicion del ultimo fotograma.
fn clic(v: &mut Ventana, d: &disposicion::Disposicion, p: (f32, f32), hwnd: windows::Win32::Foundation::HWND) -> Tras {
    if let Some(z) = d.zona_en(p) {
        let o = match z {
            Zona::Borrar => v.estado.poner_consulta(""),
            Zona::Pestana(pe) => v.estado.ir_a_pestana(pe),
            Zona::Boton(modelo::Boton::Mas) => {
                let opciones = v.opciones_de_menu();
                v.estado.abrir_menu(opciones)
            }
            Zona::Boton(b) => v.estado.boton(b),
            Zona::Ficha(i) => match v.recientes.busquedas.get(i).cloned() {
                Some(b) => v.estado.poner_consulta(&b),
                None => Orden::Nada,
            },
            Zona::QuitarFicha(i) => {
                v.recientes.quitar_busqueda(i);
                v.recientes.guardar(&v.raiz());
                Orden::Repintar
            }
            Zona::BorrarTodas => {
                v.recientes.busquedas.clear();
                v.recientes.guardar(&v.raiz());
                Orden::Repintar
            }
            Zona::Letra(l) => v.estado.poner_consulta(&format!("{l} ")),
            Zona::OpcionMenu(i) => match v.estado.menu.as_ref().and_then(|m| m.opciones.get(i)) {
                Some(o) => Orden::Hacer { accion: o.accion.clone(), recordar: false },
                None => Orden::Nada,
            },
            Zona::Conservar => match v.estado.elegido().and_then(modelo::ruta_de_captura) {
                Some(r) => Orden::Hacer {
                    accion: Accion::Pedido(resultados::pedido("conservar_captura", json!({ "ruta": r }))),
                    recordar: false,
                },
                None => Orden::Nada,
            },
            Zona::Deshacer => Orden::Deshacer,
        };
        return v.orden(o, hwnd);
    }
    // Una fila: el primer clic la elige; otro sobre la elegida la abre.
    let lineas = v.estado.lineas();
    if let Some(i) = d.fila_en(&lineas, p) {
        if i == v.estado.elegido {
            let o = v.estado.boton(modelo::Boton::Principal);
            return v.orden(o, hwnd);
        }
        v.estado.elegido = i;
        v.estado.menu = None;
    }
    Tras::Nada
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_peso_se_lee_como_en_el_explorador() {
        assert_eq!(peso_legible(412 * 1024), "412 KB");
        assert_eq!(peso_legible(1), "1 KB");
        assert_eq!(peso_legible(1_288_490), "1,2 MB");
        // Caso negativo: cero no es «0,0 MB».
        assert_eq!(peso_legible(0), "0 KB");
    }

    fn datos_de_prueba(nombre: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("pixpin-buscar-todo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("capturas")).unwrap();
        raiz
    }

    #[test]
    fn al_buscar_en_todo_salen_tambien_las_capturas_y_con_una_letra_no() {
        let raiz = datos_de_prueba("capturas");
        std::fs::write(raiz.join("capturas").join("grieta-muro.png"), b"no es un png de verdad").unwrap();
        std::fs::write(raiz.join("capturas").join("otra.png"), b"x").unwrap();
        let mut f = Fuente { raiz: raiz.clone(), datos: Datos::nuevo(raiz.clone()) };
        f.datos.vigilar = false;
        let r = resultados_de("grieta", Pestana::Todo, &mut f, &Recientes::default());
        let caps: Vec<&str> =
            r.iter().filter(|r| modelo::pestana_de(r) == Pestana::Capturas).map(|r| r.titulo.as_str()).collect();
        assert_eq!(caps, vec!["grieta-muro.png"]);
        // Caso negativo: «t grieta» es apuntar una tarea, no buscar capturas.
        let r = resultados_de("t grieta", Pestana::Todo, &mut f, &Recientes::default());
        assert!(r.iter().all(|r| modelo::pestana_de(r) != Pestana::Capturas));
        assert!(r.iter().any(|r| r.titulo.contains("grieta")), "{r:?}");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn con_la_caja_vacia_salen_los_abiertos_y_la_ultima_captura() {
        let raiz = datos_de_prueba("vacia");
        std::fs::write(raiz.join("capturas").join("ultima.png"), b"x").unwrap();
        let mut f = Fuente { raiz: raiz.clone(), datos: Datos::nuevo(raiz.clone()) };
        f.datos.vigilar = false;
        let r = resultados_de("", Pestana::Todo, &mut f, &Recientes::default());
        assert_eq!(r.first().map(|r| r.titulo.as_str()), Some("ultima.png"));
        // En «Capturas» con la caja vacia: las capturas, como `p capturas`.
        let r = resultados_de("", Pestana::Capturas, &mut f, &Recientes::default());
        assert!(r.iter().any(|r| r.titulo == "ultima.png"));
        // Caso negativo: sin nada en disco, la caja vacia no inventa nada.
        let vacia = datos_de_prueba("nada");
        let mut f = Fuente { raiz: vacia.clone(), datos: Datos::nuevo(vacia.clone()) };
        f.datos.vigilar = false;
        assert!(resultados_de("", Pestana::Todo, &mut f, &Recientes::default()).is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&vacia);
    }

    #[test]
    fn las_letras_rapidas_son_las_del_plugin() {
        let raiz = datos_de_prueba("letras");
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let l = letras(&textos, &raiz, &Opciones { atajo_capturar: Some("Ctrl Alt X".into()), ..Default::default() });
        let cuales: String = l.iter().map(|l| l.letra).collect();
        assert_eq!(cuales, "tnlgcua");
        for x in &l {
            assert!(Funcion::atajo(&x.letra.to_string()).is_some());
            assert!(!x.titulo.starts_with("buscar-todo"), "falta el texto de «{}»", x.letra);
        }
        assert_eq!(l[4].atajo.as_deref(), Some("Ctrl Alt X"));
        // Caso negativo: solo «c» lleva el atajo de capturar.
        assert!(l.iter().filter(|x| x.letra != 'c').all(|x| x.atajo.is_none()));
        assert_eq!(l[5].sub, textos.t("buscar-todo-letra-u-sub-nada"), "sin capturas lo dice");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_chapita_de_letra_sale_de_la_funcion_o_de_lo_que_crea() {
        let mut f = modelo::resultado("Tareas", "", resultados::glifo::TAREAS, Accion::Consulta("tareas ".into()));
        f.clave = Some("funcion/tareas".into());
        assert_eq!(pintar::letra_de(&f), Some('t'));
        let apuntar = modelo::resultado("Apuntar", "", resultados::glifo::ANADIR, Accion::Pedido(resultados::pedido("anadir_tarea", json!({}))));
        assert_eq!(pintar::letra_de(&apuntar), Some('t'));
        // Caso negativo: un proyecto o una funcion sin letra no llevan.
        let mut p = modelo::resultado("Thesis", "", resultados::glifo::PROYECTO, Accion::Consulta("x".into()));
        p.clave = Some("proyecto/t".into());
        assert_eq!(pintar::letra_de(&p), None);
        p.clave = Some("funcion/grabar".into());
        assert_eq!(pintar::letra_de(&p), None);
    }

    const FOTO: &str = r"C:\datos\capturas\grieta.png";

    /// Los resultados de la maqueta («grie»), hechos a mano.
    fn de_la_maqueta() -> Vec<Resultado> {
        use pixpin_lanzador::normalizar::resaltado;
        let r = |t: &str, s: &str, g: &'static str, a: Accion, k: Option<&str>| {
            let mut r = modelo::resultado(t, s, g, a);
            r.clave = k.map(str::to_string);
            r.resaltado = resaltado(t, "grie");
            r
        };
        let foto = FOTO.to_string();
        let mut c = r(
            "Grieta en el muro norte",
            "Captura · Obra Miraflores · hoy 09:12",
            resultados::glifo::IMAGEN,
            Accion::Pedido(resultados::pedido("pinear", json!({ "ruta": foto }))),
            Some("captura/grieta.png"),
        );
        c.vista_previa = Some(foto.clone());
        c.fichero = Some(foto.clone());
        c.contexto = Some(json!({ "tipo": "captura", "ruta": foto, "conservada": false }));
        c.ayuda_subtitulo = Some("«Grieta en el muro norte, revisar el lunes» — enviada al chat de Obra Miraflores".into());
        let mut t = r(
            "☐ Revisar la grieta del muro con el ingeniero",
            "Tarea · Obra Miraflores · vence el lunes · 2 adjuntos",
            resultados::glifo::PENDIENTE,
            Accion::Consulta("x".into()),
            Some("tarea/p/c/x"),
        );
        t.contexto = Some(json!({ "tipo": "tarea", "proyecto": "p", "codigo": "c" }));
        t.copiar = Some("Revisar".into());
        let mut f = r(
            "Informe de grietas – bloque B.pdf",
            "Obra Miraflores · 12 páginas · 2 sep",
            resultados::glifo::ARCHIVO,
            Accion::Consulta("x".into()),
            Some("fichero/x.pdf"),
        );
        f.fichero = Some(FOTO.replace("capturas", "obra").replace("grieta.png", "Informe de grietas – bloque B.pdf"));
        vec![
            c,
            t,
            r("Sellar las grietas antes de pintar", "Lección · Obra Miraflores · repasar hoy", resultados::glifo::LECCION, Accion::Consulta("x".into()), Some("leccion/1")),
            f,
            r(
                "Apuntar tarea: grie",
                "Tarea · Inbox · Intro: apuntarla",
                resultados::glifo::ANADIR,
                Accion::Pedido(resultados::pedido("anadir_tarea", json!({ "texto": "grie" }))),
                None,
            ),
        ]
    }

    /// **Las pantallas del buscador en PNG** (necesita GPU): el inicio, con
    /// resultados, con el menu y de dia. Deja los PNG en `PIXPIN_MUESTRAS`.
    #[test]
    #[ignore = "necesita GPU; deja PNG en PIXPIN_MUESTRAS"]
    fn muestras_del_buscador() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let raiz = datos_de_prueba("muestras");
        let letras = letras(&textos, &raiz, &Opciones { atajo_capturar: Some("Ctrl Alt X".into()), ..Default::default() });
        let busquedas: Vec<String> = ["grieta", "factura temu", "bibliografía tesis", "IEN 2024"].map(String::from).to_vec();
        let mut inicio = Estado::default();
        let mut abiertos = de_la_maqueta();
        abiertos.truncate(4);
        abiertos.iter_mut().for_each(|r| r.resaltado.clear());
        inicio.poner_resultados(abiertos);
        let mut con = Estado { consulta: "grie".into(), ..Default::default() };
        con.poner_resultados(de_la_maqueta());
        let mut menu = con.clone();
        menu.tecla(modelo::VK_DOWN, false, false, false);
        let ctx = Contexto::con("", 0);
        let opciones = pixpin_lanzador::menu::menu(menu.elegido().unwrap().contexto.as_ref().unwrap(), &ctx);
        menu.abrir_menu(opciones);
        let mut borrada = con.clone();
        borrada.se_puede_deshacer = true;
        let casos: [(&str, &Estado, Paleta, f32, Option<&str>); 6] = [
            ("buscar2-vacio", &inicio, Paleta::NOCHE, 1.0, None),
            ("buscar2-resultados", &con, Paleta::NOCHE, 1.0, None),
            ("buscar2-menu", &menu, Paleta::NOCHE, 1.0, None),
            ("buscar2-resultados-dia", &con, Paleta::DIA, 1.0, None),
            ("buscar2-vacio-150", &inicio, Paleta::NOCHE, 1.5, None),
            ("buscar2-deshacer", &borrada, Paleta::NOCHE, 1.0, Some("Captura borrada")),
        ];
        for (nombre, estado, pal, e, aviso) in casos {
            let (w, h) = ((disposicion::ANCHO as f32 * e) as u32, (disposicion::ALTO as f32 * e) as u32);
            crate::ventanita::muestra(nombre, w, h, |p, motor| {
                // Una «foto» de degradado para la captura.
                let (fw, fh) = (480u32, 270u32);
                let mut px = Vec::with_capacity((fw * fh * 4) as usize);
                for y in 0..fh {
                    for x in 0..fw {
                        px.extend_from_slice(&[(60 + x / 4) as u8, (90 + y / 3) as u8, 140, 255]);
                    }
                }
                let b = motor.bitmap_desde_pixeles(fw, fh, &px).expect("foto");
                let mut fotos = HashMap::new();
                fotos.insert(FOTO.to_string(), (b, fw, fh));
                let detalle = Detalle {
                    medidas: Some("Captura · 1920 × 1080 · 412 KB".into()),
                    caducidad: Some(("Caduca en 7 días".into(), true)),
                };
                let es_captura = estado.elegido().and_then(modelo::ruta_de_captura).is_some();
                let vista = Vista {
                    estado,
                    busquedas: &busquedas,
                    letras: &letras,
                    atajo: Some("Ctrl Espacio"),
                    fotos: &fotos,
                    detalle: if es_captura { detalle } else { Detalle::default() },
                    hay_menu: true,
                    aviso,
                    raton: (-1.0, -1.0),
                    scroll: 0.0,
                };
                let d = pintar::pintar(p, &vista, &pal, &textos, w as f32, h as f32, e);
                assert!(!d.pestanas.is_empty());
            });
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
