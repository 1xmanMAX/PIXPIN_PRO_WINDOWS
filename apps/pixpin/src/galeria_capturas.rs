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
//! donde se puede sacar a mano: nada se borra de verdad desde aqui.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, CRISTAL, FONDO, ROJO, TEXTO, centrado, dentro};

/// Lado mayor de la miniatura guardada: la celda mide 168 logicos, y al
/// 150 % son 252; con 256 no se ve pastosa y pesa 256 KB como mucho.
const LADO: u32 = 256;
/// Cuantas miniaturas se tienen en memoria a la vez, como mucho. Una
/// pantalla llena son ~25; el resto es colchon para volver atras sin
/// recargar. 160 de 256 px son ~40 MB en el peor caso.
const TOPE_EN_MEMORIA: usize = 160;
/// Medidas de la rejilla, en pixeles logicos.
const CELDA: f32 = 168.0;
const HUECO: f32 = 12.0;
const MARGEN: f32 = 16.0;
const BARRA: f32 = 56.0;
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
    ubicacion.raiz().join("capturas")
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

// ------------------------------------------------------------- la rejilla

/// Como cae la rejilla en la ventana, en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rejilla {
    columnas: usize,
    celda: f32,
    hueco: f32,
    /// Arriba a la izquierda de la primera celda, sin desplazar.
    x0: f32,
    y0: f32,
    /// Lo que se ve de la rejilla: de `y0` al borde de abajo.
    alto_vista: f32,
}

fn rejilla(ancho: f32, alto: f32, escala: f32) -> Rejilla {
    let celda = CELDA * escala;
    let hueco = HUECO * escala;
    let util = (ancho - 2.0 * MARGEN * escala).max(celda);
    let columnas = (((util + hueco) / (celda + hueco)).floor() as usize).max(1);
    // Centrada: lo que sobra a partes iguales a cada lado.
    let ocupado = columnas as f32 * celda + (columnas as f32 - 1.0) * hueco;
    Rejilla {
        columnas,
        celda,
        hueco,
        x0: ((ancho - ocupado) / 2.0).max(0.0),
        y0: BARRA * escala + 4.0 * escala,
        alto_vista: (alto - BARRA * escala - 4.0 * escala).max(0.0),
    }
}

impl Rejilla {
    fn filas(&self, n: usize) -> usize {
        n.div_ceil(self.columnas)
    }

    /// Lo que mide la rejilla entera, para saber hasta donde se desplaza.
    fn alto_total(&self, n: usize) -> f32 {
        let filas = self.filas(n) as f32;
        (filas * (self.celda + self.hueco) + self.hueco).max(0.0)
    }

    fn scroll_maximo(&self, n: usize) -> f32 {
        (self.alto_total(n) - self.alto_vista).max(0.0)
    }

    /// La celda `i` con el desplazamiento aplicado.
    fn celda(&self, i: usize, scroll: f32) -> RectF {
        let fila = (i / self.columnas) as f32;
        let col = (i % self.columnas) as f32;
        RectF {
            x: self.x0 + col * (self.celda + self.hueco),
            y: self.y0 + fila * (self.celda + self.hueco) - scroll,
            ancho: self.celda,
            alto: self.celda,
        }
    }

    /// Las capturas que se ven (y `colchon` filas mas por cada lado, para que
    /// al desplazar ya esten cargadas).
    fn visibles(&self, n: usize, scroll: f32, colchon: usize) -> Range<usize> {
        if n == 0 {
            return 0..0;
        }
        let paso = self.celda + self.hueco;
        let primera = (scroll / paso).floor().max(0.0) as usize;
        let ultima = ((scroll + self.alto_vista) / paso).ceil() as usize;
        let desde = primera.saturating_sub(colchon) * self.columnas;
        let hasta = ((ultima + colchon + 1) * self.columnas).min(n);
        desde.min(hasta)..hasta
    }
}

// ----------------------------------------------------------- miniaturas

/// Lo que devuelve el hilo de las miniaturas.
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

// --------------------------------------------------------------- la ventana

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    Carpeta,
    Mover,
    Abrir(usize),
    Pinear(usize),
    Copiar(usize),
    Borrar(usize),
}

struct Estado {
    lista: Vec<Entrada>,
    minis: HashMap<PathBuf, Mini>,
    scroll: f32,
    botones: Botones<Accion>,
    aviso: Option<(String, Instant)>,
    /// Arrastrando la ventana por su barra: donde se pulso, en pantalla, y
    /// donde estaba la ventana entonces.
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    fotograma: u64,
}

fn bucle(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(monitor.area_trabajo, 980, 720, monitor.escala_por_cien);
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
    let mut e = Estado {
        lista: listar(&dir),
        minis: HashMap::new(),
        scroll: 0.0,
        botones: Botones::default(),
        aviso: None,
        moviendo: None,
        fotograma: 0,
    };
    tracing::info!(capturas = e.lista.len(), "galeria de capturas abierta");
    let mut fecha_carpeta = fecha_de(&dir);
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
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
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => hacer(&mut e, a, textos, ubicacion, &mut vivo),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => {
                    let r = rejilla(marco.ancho as f32, marco.alto as f32, escala);
                    e.scroll = (e.scroll - m as f32 * (r.celda + r.hueco) * 0.6)
                        .clamp(0.0, r.scroll_maximo(e.lista.len()));
                }
                EventoOverlay::Tecla { vk: 0x1B, .. } => vivo = false,
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        // Las miniaturas que llegaron del hilo.
        loop {
            match hechas.try_recv() {
                Ok((ruta, mini)) => {
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
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        // La carpeta, si cambio (una captura nueva, una borrada a mano).
        if mirado.elapsed() >= Duration::from_secs(2) {
            mirado = Instant::now();
            let ahora = fecha_de(&dir);
            if ahora != fecha_carpeta {
                fecha_carpeta = ahora;
                e.lista = listar(&dir);
                pintar = true;
            }
        }
        if e.aviso.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(2_500)) {
            e.aviso = None;
            pintar = true;
        }

        if pintar {
            e.fotograma += 1;
            let r = rejilla(marco.ancho as f32, marco.alto as f32, escala);
            e.scroll = e.scroll.clamp(0.0, r.scroll_maximo(e.lista.len()));
            pedir_y_subir(&mut e, &r, &pedir, &cache, &motor);
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| pintar_todo(&mut e, p, marco, escala, textos));
                let _ = superficie.presentar();
            }
            soltar_lejanas(&mut e, &r);
            pintar = false;
        }
        pixpin_shell::overlay::esperar_eventos(Some(if e.aviso.is_some() { 250 } else { 1_000 }));
    }
    tracing::info!("galeria de capturas cerrada");
    Ok(())
}

fn fecha_de(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// Pide al hilo las miniaturas a la vista que faltan y sube a la GPU las
/// que ya llegaron. Se hace ANTES de pintar: crear bitmaps a medio dibujo
/// no se puede.
fn pedir_y_subir(
    e: &mut Estado,
    r: &Rejilla,
    pedir: &Sender<(Entrada, PathBuf)>,
    cache: &Path,
    motor: &pixpin_render::MotorRender,
) {
    let rango = r.visibles(e.lista.len(), e.scroll, 1);
    // Al reves: el hilo atiende primero lo ultimo pedido, y asi lo de arriba
    // de la vista llega antes.
    for i in rango.rev() {
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
fn soltar_lejanas(e: &mut Estado, r: &Rejilla) {
    let listas = e
        .minis
        .values()
        .filter(|m| matches!(m, Mini::Lista { .. }))
        .count();
    if listas <= TOPE_EN_MEMORIA {
        return;
    }
    let rango = r.visibles(e.lista.len(), e.scroll, 2);
    let a_la_vista: std::collections::HashSet<&PathBuf> =
        e.lista[rango].iter().map(|x| &x.ruta).collect();
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

fn hacer(e: &mut Estado, a: Accion, textos: &Catalogo, ubicacion: &Ubicacion, vivo: &mut bool) {
    let de = match a {
        Accion::Abrir(i) | Accion::Pinear(i) | Accion::Copiar(i) | Accion::Borrar(i) => {
            e.lista.get(i).map(|x| x.ruta.clone())
        }
        _ => None,
    };
    let ruta = |_: usize| de.clone();
    let fallo = |textos: &Catalogo, motivo: String| {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("motivo", motivo);
        textos.t_args("galeria-no-se-pudo", &args)
    };
    match a {
        Accion::Cerrar => *vivo = false,
        Accion::Mover => {}
        Accion::Carpeta => {
            let dir = carpeta(ubicacion);
            let _ = std::fs::create_dir_all(&dir);
            if let Err(err) = pixpin_shell::abrir(&dir) {
                e.aviso = Some((fallo(textos, err.to_string()), Instant::now()));
            }
        }
        Accion::Abrir(i) => {
            if let Some(r) = ruta(i)
                && let Err(err) = pixpin_shell::abrir(&r)
            {
                e.aviso = Some((fallo(textos, err.to_string()), Instant::now()));
            }
        }
        Accion::Pinear(i) => {
            // Por el mismo camino que «Abrir con PixPin»: la ventana
            // principal, que es la que tiene los pines, la pinea.
            if let Some(r) = ruta(i) {
                let ok = pixpin_shell::mensajero::enviar_ficheros(&[r]);
                e.aviso = Some((
                    if ok {
                        textos.t("galeria-pineada")
                    } else {
                        fallo(textos, "PixPin".into())
                    },
                    Instant::now(),
                ));
            }
        }
        Accion::Copiar(i) => {
            if let Some(r) = ruta(i) {
                let hecho = pixpin_codec::imagen::cargar(&r)
                    .map_err(|err| err.to_string())
                    .and_then(|img| pixpin_codec::copiar_imagen(&img).map_err(|err| err.to_string()));
                e.aviso = Some((
                    match hecho {
                        Ok(()) => textos.t("galeria-copiada"),
                        Err(m) => fallo(textos, m),
                    },
                    Instant::now(),
                ));
            }
        }
        Accion::Borrar(i) => {
            if let Some(r) = ruta(i) {
                match a_la_papelera(ubicacion.raiz(), &r) {
                    Ok(destino) => {
                        tracing::info!(ruta = %r.display(), destino = %destino.display(), "captura a la papelera");
                        e.lista.remove(i);
                        e.minis.remove(&r);
                        e.aviso = Some((textos.t("galeria-borrada"), Instant::now()));
                    }
                    Err(err) => e.aviso = Some((fallo(textos, err.to_string()), Instant::now())),
                }
            }
        }
    }
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    let r = rejilla(w, h, escala);
    let n = e.lista.len();

    if n == 0 {
        let t = textos.t("galeria-vacia");
        let tam = 15.0 * escala;
        let (tw, _) = p.medir_texto(&t, tam);
        p.texto_linea(
            &t,
            ((w - tw) / 2.0).max(MARGEN * escala),
            h / 2.0,
            tam,
            w - 2.0 * MARGEN * escala,
            APAGADO,
        );
    }

    let encima = e.botones.raton;
    for i in r.visibles(n, e.scroll, 0) {
        let c = r.celda(i, e.scroll);
        if c.y + c.alto < r.y0 || c.y > h {
            continue;
        }
        let entrada = &e.lista[i];
        p.rellenar_redondeado(c, 10.0 * escala, CRISTAL);
        let hueco = 4.0 * escala;
        let dentro_c = RectF {
            x: c.x + hueco,
            y: c.y + hueco,
            ancho: c.ancho - 2.0 * hueco,
            alto: c.alto - 2.0 * hueco,
        };
        match e.minis.get(&entrada.ruta) {
            Some(Mini::Lista {
                bitmap: Some(b),
                imagen,
                ..
            }) => crate::miniaturas::pintar_recortado(p, b, dentro_c, imagen.ancho, imagen.alto),
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
        // Todo el recuadro abre; los botones de encima, apuntados despues,
        // le ganan.
        e.botones.zona(c, Accion::Abrir(i));
        // Con el raton en la barra no: la celda que asoma por debajo no es
        // la que se esta mirando.
        if dentro(c, encima) && encima.1 >= r.y0 {
            pintar_acciones(e, p, c, i, escala);
        }
    }

    // La barra de arriba, por encima de lo que se desplazo bajo ella. Su
    // zona de mover se apunta DESPUES de las celdas (gana a la que asome por
    // debajo) y ANTES de sus botones (que le ganan a ella).
    let barra = BARRA * escala;
    let caja_barra = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: barra,
    };
    p.rellenar(caja_barra, CRISTAL);
    e.botones.zona(caja_barra, Accion::Mover);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuantas", n.to_string());
    let titulo = textos.t_args("galeria-titulo-cuantas", &args);
    p.texto(&titulo, MARGEN * escala, 16.0 * escala, 18.0 * escala, TEXTO);
    let alto_b = 36.0 * escala;
    let yb = (barra - alto_b) / 2.0;
    let cerrar = RectF {
        x: w - MARGEN * escala - alto_b,
        y: yb,
        ancho: alto_b,
        alto: alto_b,
    };
    e.botones.boton(p, cerrar, Accion::Cerrar, "", None, escala);
    p.icono(&mi::CLOSE, encoger(cerrar, 8.0 * escala), TEXTO);
    let rotulo = textos.t("galeria-abrir-carpeta");
    let (tw, _) = p.medir_texto(&rotulo, 15.0 * escala);
    let carpeta_b = RectF {
        x: cerrar.x - 10.0 * escala - (tw + 28.0 * escala),
        y: yb,
        ancho: tw + 28.0 * escala,
        alto: alto_b,
    };
    e.botones.boton(p, carpeta_b, Accion::Carpeta, &rotulo, None, escala);

    if let Some((t, _)) = &e.aviso {
        let tam = 14.0 * escala;
        let (tw, th) = p.medir_texto(t, tam);
        let caja = RectF {
            x: (w - tw) / 2.0 - 14.0 * escala,
            y: h - th - 34.0 * escala,
            ancho: tw + 28.0 * escala,
            alto: th + 16.0 * escala,
        };
        p.rellenar_redondeado(caja, 8.0 * escala, Color { a: 0.92, ..Color::NEGRO });
        p.texto(t, caja.x + 14.0 * escala, caja.y + 8.0 * escala, tam, TEXTO);
    }
}

/// Los cuatro botones que salen encima de la captura bajo el raton.
fn pintar_acciones(e: &mut Estado, p: &Pintor, c: RectF, i: usize, escala: f32) {
    let lado = 34.0 * escala;
    let hueco = 6.0 * escala;
    let franja = RectF {
        x: c.x,
        y: c.y + c.alto - lado - 2.0 * hueco,
        ancho: c.ancho,
        alto: lado + 2.0 * hueco,
    };
    p.rellenar(franja, Color { a: 0.55, ..Color::NEGRO });
    let acciones = [
        (Accion::Abrir(i), &mi::OPEN_IN_NEW, TEXTO),
        (Accion::Pinear(i), &mi::PUSH_PIN, TEXTO),
        (Accion::Copiar(i), &mi::CONTENT_COPY, TEXTO),
        (Accion::Borrar(i), &mi::DELETE, ROJO),
    ];
    let total = acciones.len() as f32 * lado + (acciones.len() as f32 - 1.0) * hueco;
    let mut x = c.x + (c.ancho - total) / 2.0;
    for (accion, icono, color) in acciones {
        let caja = RectF {
            x,
            y: franja.y + hueco,
            ancho: lado,
            alto: lado,
        };
        e.botones.boton(p, caja, accion, "", None, escala);
        p.icono(icono, encoger(caja, 7.0 * escala), color);
        x += lado + hueco;
    }
}

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
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
        assert_eq!(nombres, ["captura-0005.png", "captura-0191.png", "captura-0190.png"]);
    }

    #[test]
    fn a_igual_hora_gana_el_nombre_mas_alto() {
        let mut v = vec![entrada("captura-0001.png", 5), entrada("captura-0002.png", 5)];
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
        let dir = std::env::temp_dir().join(format!("pixpin-galeria-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        let raiz = std::env::temp_dir().join(format!("pixpin-galeria-pap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
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
    fn la_cache_cambia_si_cambia_la_captura() {
        let cache = Path::new("c");
        let a = entrada("captura-0001.png", 10);
        let mut b = a.clone();
        b.bytes = 11;
        assert_ne!(ruta_en_cache(cache, &a), ruta_en_cache(cache, &b));
        let mut c = a.clone();
        c.cuando += Duration::from_secs(1);
        assert_ne!(ruta_en_cache(cache, &a), ruta_en_cache(cache, &c));
    }

    #[test]
    fn la_rejilla_llena_el_ancho_y_nunca_tiene_cero_columnas() {
        let r = rejilla(980.0, 720.0, 1.0);
        assert_eq!(r.columnas, 5, "(980-32+12)/(168+12) = 5,3");
        let estrecha = rejilla(100.0, 720.0, 1.0);
        assert_eq!(estrecha.columnas, 1, "caso negativo: nunca cero columnas");
        assert!(r.x0 >= MARGEN, "centrada, con margen");
    }

    #[test]
    fn solo_se_piden_las_filas_a_la_vista() {
        // Mil capturas: lo virtual es que se pidan unas pocas filas, no mil.
        let r = rejilla(980.0, 720.0, 1.0);
        let v = r.visibles(1000, 0.0, 0);
        assert_eq!(v.start, 0);
        assert!(v.len() <= r.columnas * 5, "a la vista: {v:?}");
        // Desplazado hasta el final, el rango acaba en la ultima.
        let fondo = r.visibles(1000, r.scroll_maximo(1000), 0);
        assert_eq!(fondo.end, 1000);
        assert!(fondo.start > 900);
        // Sin capturas, nada.
        assert!(r.visibles(0, 0.0, 1).is_empty());
    }

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_galeria() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let mut e = Estado {
            lista: (0..13)
                .map(|i| entrada(&format!("captura-{i:04}.png"), i))
                .chain(std::iter::once(entrada("grabacion.mp4", 99)))
                .collect(),
            minis: HashMap::new(),
            scroll: 0.0,
            botones: Botones::default(),
            aviso: Some((textos.t("galeria-copiada"), Instant::now())),
            moviendo: None,
            fotograma: 0,
        };
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 980,
            alto: 720,
        };
        // El raton sobre la segunda celda: salen sus cuatro botones.
        let r = rejilla(980.0, 720.0, 1.0);
        let c = r.celda(1, 0.0);
        e.botones.raton = (c.x + 20.0, c.y + 20.0);
        crate::ventanita::muestra("galeria-capturas", 980, 720, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
    }

    #[test]
    fn la_miniatura_se_reduce_y_queda_en_la_cache() {
        let dir = std::env::temp_dir().join(format!("pixpin-galeria-mini-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("captura-0001.png");
        let grande = ImagenRgba {
            ancho: 600,
            alto: 300,
            pixeles: [10u8, 200, 30, 255].repeat(600 * 300),
        };
        pixpin_codec::guardar(&grande, &ruta, pixpin_codec::FormatoImagen::Png).unwrap();
        let en_cache = dir.join("mini.png");
        let m = leer_reducida(&ruta, &en_cache).expect("se lee");
        assert_eq!((m.ancho, m.alto), (LADO, LADO / 2), "lado mayor a LADO, sin deformar");
        assert!(en_cache.exists(), "y queda guardada para la proxima vez");
        // La segunda sale de la cache aunque la captura ya no este.
        std::fs::remove_file(&ruta).unwrap();
        assert!(leer_reducida(&ruta, &en_cache).is_some());
        // Caso negativo: sin captura ni cache no hay miniatura (ni panico).
        assert!(leer_reducida(&ruta, &dir.join("otra.png")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn con_pocas_capturas_no_hay_nada_que_desplazar() {
        let r = rejilla(980.0, 720.0, 1.0);
        assert_eq!(r.scroll_maximo(3), 0.0);
        assert!(r.scroll_maximo(100) > 0.0);
    }
}
