//! **Las hojas de Excel como un pin** (10-oct-2026): un `.xlsx` o `.xlsm`
//! abierto con PixPin (doble clic, «Abrir con», la vista rapida del
//! espacio, un documento de una nota) se ve en una ventana como la de los
//! PDF y los Word (`lector_pin`): su barra fuera con el asa, el nombre,
//! pantalla completa, el pin y cerrar; Esc cierra (o sale de pantalla
//! completa) y F11 la alterna.
//!
//! Lo lee `pixpin-hojas` y lo pinta `pixpin_render::hoja_office`, como lo
//! pinta Excel: anchos y altos, estilos, formatos de numero y fecha con la
//! configuracion regional del usuario, combinaciones, paneles inmovilizados,
//! imagenes y la tira de pestanas.
//!
//! **Se abre al instante**: se leen las filas de la primera pantalla y se
//! pinta; el resto llega por tandas desde otro hilo (`leer_aparte`), que
//! despierta a la ventana con cada una. La barra de desplazamiento ya mide
//! la hoja entera desde el principio (la `dimension` del libro).
//!
//! Lo que se puede hacer: rueda (Mayus+rueda, de lado), Ctrl+rueda acerca
//! alrededor del raton (Ctrl+0 vuelve al 100 %), arrastrar las barras finas,
//! flechas, RePag/AvPag, Inicio, Ctrl+Inicio/Fin y Ctrl+flecha como en
//! Excel; clic elige una celda, arrastrar un rango, los encabezados eligen
//! filas o columnas; Ctrl+C copia lo elegido como texto con tabuladores (lo
//! que se ve en cada celda); un clic en un enlace lo abre (web o un sitio
//! del libro); las pestanas de abajo o Ctrl+RePag/AvPag cambian de hoja.
//! Arriba, una linea como la barra de formulas: la celda activa y lo que
//! tiene.
//!
//! Un libro que no se puede leer (un `.xls` renombrado, uno roto, uno
//! binario) se abre con su programa, como antes.

mod logica;

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Punto;
use pixpin_hojas::{Hoja, LectorHoja, Libro, Rango, Regional, Rejilla, Tanda, Valor, Visibilidad, Vista, nombre_columna};
use pixpin_render::hoja_office::{self, CacheHoja, OpcionesHoja, Pestana};
use pixpin_render::icono::material;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};

use crate::lector_pin::{AccionPin, BotonLector, PinLector};
use crate::overlay::Recursos;
use logica::{AccionTecla, Enlace, Mov, Seleccion};
pub use logica::{se_abre, se_abre_fichero};

/// Medidas en pixeles logicos.
const ALTO_FORMULA: f32 = 26.0;
const ALTO_PESTANAS: f32 = 28.0;
/// Las barras de desplazamiento: finas, y algo mas con el raton encima.
const BARRA: f32 = 6.0;
const BARRA_ENCIMA: f32 = 10.0;
const PULGAR_MINIMO: f32 = 28.0;
const TAM_TEXTO: f32 = 12.5;
/// Celdas por tanda del hilo que lee (una tanda tarda unos milisegundos).
const TANDA_CELDAS: usize = 150_000;
/// Un clic en una celda con enlace lo abre si se suelta antes de esto.
const CLIC_DE_ENLACE: Duration = Duration::from_millis(600);
const AVISO: Duration = Duration::from_millis(1800);

const FONDO_BARRA: Color = Color { r: 0.953, g: 0.953, b: 0.953, a: 1.0 };
const LINEA: Color = Color { r: 0.831, g: 0.831, b: 0.831, a: 1.0 };
const TEXTO: Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
const TENUE: Color = Color { r: 0.42, g: 0.42, b: 0.42, a: 1.0 };
const ENLACE: Color = Color { r: 0.02, g: 0.39, b: 0.76, a: 1.0 };
const PULGAR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.28 };
const PULGAR_ENCIMA: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 };

/// Botones propios en la barra del pin.
const BOTON_COPIAR: u8 = 1;
const BOTON_SU_PROGRAMA: u8 = 2;
/// «Editar»: la misma ventana pasa a la hoja editable (`hoja_pin`). Solo en
/// las hojas del chat, que es donde se guarda lo editado.
const BOTON_EDITAR: u8 = 3;
/// F2, como en Excel, y la `E` de «Editar».
const VK_F2: u32 = 0x71;
const VK_E: u32 = 0x45;

/// Como acaba el modo ver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fin {
    Cerrar,
    /// Pasar a editar, con la hoja que se estaba viendo.
    Editar(usize),
}

/// **La ventana del pin**, que sigue siendo la misma al pasar de ver a
/// editar y de vuelta: su superficie, la barra del pin y donde esta.
pub(crate) struct Marco {
    pub ventana: VentanaOverlay,
    pub superficie: Superficie,
    pub pin: PinLector,
    pub area: pixpin_geom::Rect,
    pub e: f32,
    pub nombre: String,
}

/// **Una hoja del chat** (un Excel adjunto o una tabla) como pin, en modo
/// ver y con «Editar» en su barra (`hoja_pin`). En su propio hilo.
pub(crate) fn lanzar_editable(idioma: Idioma, editable: crate::hoja_pin::Editable) {
    let t0 = Instant::now();
    let lanzado = std::thread::Builder::new().name("visor-hoja".into()).spawn(move || ver_editable(idioma, editable, t0));
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del visor de hojas");
    }
}

fn ver_editable(idioma: Idioma, mut ed: crate::hoja_pin::Editable, t0: Instant) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut libro = match ed.libro_para_ver(regional_del_usuario(idioma)) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(%e, "el visor de hojas no puede ver esa hoja del chat");
            if let Some(r) = ed.ruta_original()
                && let Err(e) = pixpin_shell::abrir(&r)
            {
                tracing::warn!(?e, ruta = %r.display(), "tampoco se pudo abrir con su programa");
            }
            return;
        }
    };
    tracing::info!(ms = ms(t0), hojas = libro.hojas.len(), "visor de hojas: hoja del chat abierta");
    let textos = Catalogo::nuevo(idioma);
    let ruta = ed.ruta_original();
    let nombre = ed.nombre();
    let hecho = crate::dispositivo_perdido::con_recursos("visor-hoja", |r| {
        abrir(r, &textos, &mut libro, ruta.as_deref(), &nombre, t0, Some(&mut ed))
    });
    if let Err(e) = hecho {
        tracing::warn!(?e, "no se pudo abrir el visor de hojas");
    }
}

/// Abre el libro en su visor, en su propio hilo. Si no se puede leer, con
/// su programa.
pub fn lanzar(idioma: Idioma, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let t0 = Instant::now();
    let lanzado = std::thread::Builder::new().name("visor-hoja".into()).spawn(move || ver(idioma, &ruta, t0));
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del visor de hojas");
    }
}

/// Lo que hace el hilo del visor: leer el libro y tener la ventana abierta
/// hasta que se cierre.
fn ver(idioma: Idioma, ruta: &Path, t0: Instant) {
    // WIC (las imagenes del libro) necesita COM en este hilo.
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut libro = match Libro::abrir(ruta, regional_del_usuario(idioma)) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(%e, ruta = %ruta.display(), "el visor de hojas no puede leer el libro; se abre con su programa");
            if let Err(e) = pixpin_shell::abrir(ruta) {
                tracing::warn!(?e, ruta = %ruta.display(), "tampoco se pudo abrir con su programa");
            }
            return;
        }
    };
    tracing::info!(
        ms = ms(t0),
        zip = libro.tiempos.zip,
        libro = libro.tiempos.libro,
        estilos = libro.tiempos.estilos,
        textos = libro.tiempos.textos,
        hojas = libro.hojas.len(),
        "visor de hojas: libro abierto"
    );
    let textos = Catalogo::nuevo(idioma);
    let nombre = crate::lector_pin::nombre_limpio(&pixpin_docs::nombre(ruta));
    let hecho = crate::dispositivo_perdido::con_recursos("visor-hoja", |r| abrir(r, &textos, &mut libro, Some(ruta), &nombre, t0, None));
    if let Err(e) = hecho {
        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el visor de hojas");
        let _ = pixpin_shell::abrir(ruta);
    }
}

fn ms(t: Instant) -> u64 {
    t.elapsed().as_millis() as u64
}

/// Los nombres del idioma de PixPin con los separadores y la fecha corta
/// que el usuario tiene en Windows.
fn regional_del_usuario(idioma: Idioma) -> Regional {
    let base = match idioma {
        Idioma::Ingles => Regional::en_us(),
        Idioma::Espanol => Regional::es_pe(),
    };
    logica::regional_de(base, &pixpin_shell::regional::del_usuario())
}

// ---------------------------------------------------------------------------
// El estado

/// Una hoja ya abierta, con donde se estaba mirando.
struct Abierta {
    hoja: Hoja,
    rejilla: Rejilla,
    /// Lo desplazado (px de hoja al 100 %), como `Vista::x`/`y`.
    x: f64,
    y: f64,
    zoom: f64,
    sel: Seleccion,
    /// Las tandas que llegan del hilo que lee el resto.
    tandas: Option<Receiver<Tanda>>,
    abierta_en: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Eje {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesto {
    /// Arrastrando para elegir un rango (y donde se pulso, por si era un
    /// clic en un enlace).
    Elegir { pulsada: (u32, u32), cuando: Instant, ctrl: bool },
    /// Eligiendo filas o columnas enteras por los encabezados.
    Encabezado(Eje),
    /// Llevando el pulgar de una barra; `agarre`: donde se cogio, desde su
    /// principio.
    Barra { eje: Eje, agarre: f32 },
}

/// Una barra de desplazamiento pintada: su pista y su pulgar, en pantalla.
#[derive(Debug, Clone, Copy)]
struct Barra {
    pista: RectF,
    pulgar: RectF,
    min: f64,
    max: f64,
}

/// Donde va cada cosa en la ventana.
#[derive(Debug, Clone, Copy)]
struct Zonas {
    formula: RectF,
    hoja: RectF,
    pestanas: RectF,
}

struct Estado<'l> {
    libro: &'l Libro,
    hojas: Vec<Option<Abierta>>,
    activa: usize,
    cache: CacheHoja,
    digito: Option<f64>,
    ppp: f64,
    /// Pixeles por pixel logico.
    e: f32,
    ancho: f32,
    alto: f32,
    pestanas: Vec<Pestana>,
    desde_pestana: usize,
    gesto: Option<Gesto>,
    raton: (f32, f32),
    encima_de: Option<Eje>,
    aviso: Option<(String, Instant)>,
    hwnd: isize,
}

fn zonas(ancho: f32, alto: f32, e: f32) -> Zonas {
    let (af, ap) = ((ALTO_FORMULA * e).round(), (ALTO_PESTANAS * e).round());
    Zonas {
        formula: RectF { x: 0.0, y: 0.0, ancho, alto: af },
        hoja: RectF { x: 0.0, y: af, ancho, alto: (alto - af - ap).max(0.0) },
        pestanas: RectF { x: 0.0, y: (alto - ap).max(af), ancho, alto: ap },
    }
}

fn dentro(r: &RectF, (x, y): (f32, f32)) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.ancho && y < r.y + r.alto
}

/// La vista con que se pinta la hoja en `caja` y la zona de sus celdas
/// (sin encabezados).
fn vista_de(ab: &Abierta, caja: RectF) -> (Vista, RectF) {
    let v = Vista { ancho: caja.ancho as f64, alto: caja.alto as f64, escala: ab.zoom, x: ab.x, y: ab.y };
    let celdas = hoja_office::zona_de_celdas(&ab.hoja, &ab.rejilla, &v, caja, true);
    (v, celdas)
}

/// La vista de las cuentas de la rejilla: del tamano de las celdas.
fn vista_celdas(v: &Vista, celdas: RectF) -> Vista {
    Vista { ancho: celdas.ancho as f64, alto: celdas.alto as f64, ..*v }
}

/// La ultima fila y columna usadas (con la `dimension` mientras se lee).
fn ultima(h: &Hoja) -> (u32, u32) {
    let d = h.cabecera.dimension;
    (h.max_fila().max(d.map_or(0, |d| d.fila1)), h.max_col.max(d.map_or(0, |d| d.col1)))
}

fn lleno(h: &Hoja, f: u32, c: u32) -> bool {
    h.celda(f, c).is_some_and(|c| c.valor != Valor::Vacio)
}

/// Lo que se ve (px de hoja) de la zona que se desplaza, en cada eje.
fn visible(ab: &Abierta, celdas: RectF) -> (f64, f64) {
    let z = ab.zoom.max(0.01);
    let fx = ab.rejilla.x(ab.rejilla.columnas_fijas);
    let fy = ab.rejilla.y(ab.rejilla.filas_fijas);
    ((celdas.ancho as f64 / z - fx).max(0.0), (celdas.alto as f64 / z - fy).max(0.0))
}

/// Hasta donde se desplaza cada eje: ((min x, max x), (min y, max y)).
fn limites_de(ab: &Abierta, celdas: RectF) -> ((f64, f64), (f64, f64)) {
    let (w, h) = ab.rejilla.tamano_usado(&ab.hoja);
    let r = &ab.rejilla;
    let lx = logica::limites(r.x(r.columnas_fijas), w, celdas.ancho as f64, ab.zoom, 2.0 * r.ancho_defecto);
    let ly = logica::limites(r.y(r.filas_fijas), h, celdas.alto as f64, ab.zoom, 4.0 * r.alto_defecto);
    (lx, ly)
}

fn acotar(ab: &mut Abierta, celdas: RectF) {
    let ((x0, x1), (y0, y1)) = limites_de(ab, celdas);
    ab.x = ab.x.clamp(x0, x1);
    ab.y = ab.y.clamp(y0, y1);
}

/// Mueve la vista lo justo para que se vea la celda (si no esta en lo
/// inmovilizado, que siempre se ve).
fn asegurar_visible(ab: &mut Abierta, celdas: RectF, (f, c): (u32, u32)) {
    let (vx, vy) = visible(ab, celdas);
    let r = &ab.rejilla;
    if f >= r.filas_fijas {
        ab.y = logica::para_ver(ab.y, vy, r.y(f), r.y(f + 1));
    }
    if c >= r.columnas_fijas {
        ab.x = logica::para_ver(ab.x, vx, r.x(c), r.x(c + 1));
    }
    acotar(ab, celdas);
}

/// Las barras de desplazamiento que hacen falta, en pantalla.
fn barras(ab: &Abierta, celdas: RectF, e: f32, encima: Option<Eje>) -> (Option<Barra>, Option<Barra>) {
    let ((x0, x1), (y0, y1)) = limites_de(ab, celdas);
    let (vx, vy) = visible(ab, celdas);
    let hay_v = y1 > y0;
    let hay_h = x1 > x0;
    let grosor = |eje| if encima == Some(eje) { BARRA_ENCIMA * e } else { BARRA * e };
    let margen = 2.0 * e;
    let (gv, gh) = (grosor(Eje::Vertical), grosor(Eje::Horizontal));
    let v = hay_v.then(|| {
        let pista = RectF {
            x: celdas.x + celdas.ancho - gv - margen,
            y: celdas.y + margen,
            ancho: gv,
            alto: (celdas.alto - 2.0 * margen - if hay_h { BARRA_ENCIMA * e } else { 0.0 }).max(0.0),
        };
        let (d, l) = logica::pulgar(pista.alto, ab.y, y0, y1, vy, PULGAR_MINIMO * e);
        Barra { pista, pulgar: RectF { y: pista.y + d, alto: l, ..pista }, min: y0, max: y1 }
    });
    let h = hay_h.then(|| {
        let pista = RectF {
            x: celdas.x + margen,
            y: celdas.y + celdas.alto - gh - margen,
            ancho: (celdas.ancho - 2.0 * margen - if hay_v { BARRA_ENCIMA * e } else { 0.0 }).max(0.0),
            alto: gh,
        };
        let (d, l) = logica::pulgar(pista.ancho, ab.x, x0, x1, vx, PULGAR_MINIMO * e);
        Barra { pista, pulgar: RectF { x: pista.x + d, ancho: l, ..pista }, min: x0, max: x1 }
    });
    (v, h)
}

/// Donde se puede coger una barra: un poco mas ancha que lo pintado.
fn zona_de_barra(b: &Barra, eje: Eje, e: f32) -> RectF {
    let extra = 6.0 * e;
    match eje {
        Eje::Vertical => RectF { x: b.pista.x - extra, ancho: b.pista.ancho + extra + 2.0 * e, ..b.pista },
        Eje::Horizontal => RectF { y: b.pista.y - extra, alto: b.pista.alto + extra + 2.0 * e, ..b.pista },
    }
}

/// Lee el resto de la hoja en otro hilo y manda las tandas; cada una
/// despierta a la ventana.
fn leer_aparte(mut lector: LectorHoja, hwnd: isize) -> Receiver<Tanda> {
    let (tx, rx) = std::sync::mpsc::channel();
    let lanzado = std::thread::Builder::new().name("visor-hoja-leer".into()).spawn(move || {
        let mut vacias = 0;
        loop {
            let mut t = lector.leer(u32::MAX, TANDA_CELDAS);
            let vacia = t.filas.is_empty() && t.celdas.is_empty() && t.cola.is_none() && t.cabecera.is_none();
            vacias = if vacia { vacias + 1 } else { 0 };
            // Un lector que ya no avanza (un ZIP cortado): se da por acabada.
            if vacias >= 3 || lector.terminado() {
                t.fin = true;
            }
            let fin = t.fin;
            if tx.send(t).is_err() {
                break;
            }
            pixpin_shell::overlay::despertar(hwnd);
            if fin {
                break;
            }
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo que lee la hoja");
    }
    rx
}

impl<'l> Estado<'l> {
    fn abierta(&self) -> &Abierta {
        self.hojas[self.activa].as_ref().expect("la hoja activa esta abierta")
    }

    fn abierta_mut(&mut self) -> &mut Abierta {
        self.hojas[self.activa].as_mut().expect("la hoja activa esta abierta")
    }

    fn zonas(&self) -> Zonas {
        zonas(self.ancho, self.alto, self.e)
    }

    /// La vista y las celdas de la hoja activa ahora.
    fn vista(&self) -> (Vista, RectF) {
        vista_de(self.abierta(), self.zonas().hoja)
    }

    /// **Abre la hoja `i`** si no lo estaba: lee lo de la primera pantalla
    /// aqui (milisegundos) y el resto en otro hilo.
    fn abrir_hoja(&mut self, i: usize, motor: &MotorRender) {
        if self.hojas.get(i).is_none_or(|h| h.is_some()) {
            return;
        }
        let t = Instant::now();
        let libro = self.libro;
        let mut hoja = libro.abrir_hoja(i);
        // La cabecera (paneles, zoom, primera celda) y la primera fila.
        hoja.cargar_hasta_fila(0);
        let vh = hoja.vista().clone();
        let zoom = logica::zoom_guardado(vh.zoom);
        // Las filas que caben, contando con filas bajas (12 px al 100 %).
        let alto_hoja = self.zonas().hoja.alto as f64;
        let caben = (alto_hoja / (12.0 * self.ppp / 96.0 * zoom)).ceil() as u32 + 2;
        let hasta = vh.primera_fila.max(vh.filas_fijas) + caben;
        hoja.cargar_hasta_fila(hasta);
        if self.digito.is_none() {
            self.digito = self.cache.ancho_digito(motor, &libro.estilos.fuente_normal, self.ppp);
        }
        let rejilla = Rejilla::nueva_en(&hoja, &libro.estilos.fuente_normal, self.digito, self.ppp);
        let tandas = hoja.tomar_lector().map(|l| leer_aparte(l, self.hwnd));
        let sel = vh.celda_activa.map_or(Seleccion::en(vh.filas_fijas, vh.columnas_fijas), |(f, c)| Seleccion::en(f, c));
        let x = rejilla.x(vh.primera_columna.max(vh.columnas_fijas));
        let y = rejilla.y(vh.primera_fila.max(vh.filas_fijas));
        tracing::info!(
            ms = ms(t),
            hoja = %hoja.nombre,
            filas = hoja.filas.len(),
            hasta,
            completa = hoja.completa,
            "visor de hojas: primera pantalla leida"
        );
        let mut ab = Abierta { hoja, rejilla, x, y, zoom, sel, tandas, abierta_en: t };
        let (_, celdas) = vista_de(&ab, self.zonas().hoja);
        acotar(&mut ab, celdas);
        self.hojas[i] = Some(ab);
    }

    fn cambiar_hoja(&mut self, i: usize, motor: &MotorRender) {
        if i >= self.libro.hojas.len() || i == self.activa {
            return;
        }
        self.abrir_hoja(i, motor);
        self.activa = i;
        self.gesto = None;
        if i < self.desde_pestana {
            self.desde_pestana = i;
        } else if !self.pestanas.iter().any(|p| p.hoja == i) {
            // Fuera de la tira por la derecha: se corre hasta ella.
            self.desde_pestana = i.saturating_sub(2);
        }
    }

    /// La siguiente (o anterior) hoja visible.
    fn hoja_vecina(&self, adelante: bool) -> Option<usize> {
        let n = self.libro.hojas.len();
        let visible = |i: usize| self.libro.hojas[i].visibilidad == Visibilidad::Visible;
        if adelante { (self.activa + 1..n).find(|&i| visible(i)) } else { (0..self.activa).rev().find(|&i| visible(i)) }
    }

    /// **Recoge las tandas que llegaron** de todas las hojas. `true` si
    /// cambio algo de la activa.
    fn recibir(&mut self) -> bool {
        let mut cambio_activa = false;
        for (i, slot) in self.hojas.iter_mut().enumerate() {
            let Some(ab) = slot.as_mut() else { continue };
            let Some(rx) = ab.tandas.as_ref() else { continue };
            let mut llego = false;
            let mut cortado = false;
            loop {
                match rx.try_recv() {
                    Ok(t) => {
                        ab.hoja.recibir(t);
                        llego = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        cortado = true;
                        break;
                    }
                }
            }
            if llego {
                ab.rejilla.actualizar(&ab.hoja);
                cambio_activa |= i == self.activa;
            }
            if ab.hoja.completa {
                ab.tandas = None;
                tracing::info!(
                    ms = ms(ab.abierta_en),
                    hoja = %ab.hoja.nombre,
                    filas = ab.hoja.filas.len(),
                    celdas = ab.hoja.celdas.len(),
                    "visor de hojas: hoja leida entera"
                );
            } else if cortado {
                ab.tandas = None;
                tracing::warn!(hoja = %ab.hoja.nombre, "el hilo que leia la hoja se corto antes del final");
            }
        }
        cambio_activa
    }

    fn cargando(&self) -> bool {
        self.hojas.iter().flatten().any(|a| a.tandas.is_some())
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some((texto, Instant::now() + AVISO));
    }

    // -----------------------------------------------------------------------
    // Acciones

    fn copiar(&mut self, textos: &Catalogo) {
        let ab = self.abierta();
        let r = ab.sel.rango();
        // Una fila o columna entera elegida: solo hasta lo usado.
        let (uf, uc) = ultima(&ab.hoja);
        let r = Rango { fila1: r.fila1.min(uf.max(r.fila0)), col1: r.col1.min(uc.max(r.col0)), ..r };
        let celdas = logica::celdas_de(r);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("celdas", celdas);
        if celdas > logica::MAXIMO_A_COPIAR {
            let t = textos.t_args("hoja-demasiado", &args);
            self.avisar(t);
            return;
        }
        let (libro, hoja) = (self.libro, &ab.hoja);
        let t = Instant::now();
        let texto = logica::tsv(r, |f, c| hoja.celda(f, c).and_then(|c| libro.contenido(hoja, c)).map(|x| x.mostrado.texto).unwrap_or_default());
        match pixpin_codec::copiar_texto(&texto) {
            Ok(()) => {
                tracing::info!(celdas, bytes = texto.len(), ms = ms(t), "visor de hojas: copiado");
                let t = textos.t_args("hoja-copiado", &args);
                self.avisar(t);
            }
            Err(e) => tracing::warn!(?e, "no se pudo copiar la seleccion"),
        }
    }

    /// Abre el enlace de la celda, si tiene. `true` si tenia.
    fn abrir_enlace(&mut self, (f, c): (u32, u32), textos: &Catalogo, motor: &MotorRender) -> bool {
        let Some(hv) = self.abierta().hoja.hipervinculo_en(f, c) else { return false };
        let externo = hv.externo;
        match logica::enlace_de(&hv.destino, externo) {
            Some(Enlace::Web(url)) => {
                tracing::info!("visor de hojas: enlace abierto");
                if let Err(e) = pixpin_shell::abrir(Path::new(&url)) {
                    tracing::warn!(?e, "no se pudo abrir el enlace de la hoja");
                    self.avisar(textos.t("hoja-enlace-no-abre"));
                }
            }
            Some(Enlace::Sitio { hoja, celda }) => {
                if let Some(nombre) = hoja
                    && let Some(i) = self.libro.hojas.iter().position(|h| h.nombre.eq_ignore_ascii_case(&nombre))
                {
                    self.cambiar_hoja(i, motor);
                }
                if let Some((f, c)) = celda {
                    let caja = self.zonas().hoja;
                    let ab = self.abierta_mut();
                    ab.sel = Seleccion::en(f, c);
                    let (_, celdas) = vista_de(ab, caja);
                    asegurar_visible(ab, celdas, (f, c));
                }
            }
            None => tracing::info!(externo, "visor de hojas: enlace que no se abre"),
        }
        true
    }

    fn mover(&mut self, m: Mov, extender: bool) {
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        let (_, celdas) = vista_de(ab, caja);
        let (_, vy) = visible(ab, celdas);
        let filas_por_pagina = (vy / ab.rejilla.alto_defecto.max(1.0)).floor().max(1.0) as u32;
        let hoja = &ab.hoja;
        let lleno_en = |f: u32, c: u32| lleno(hoja, f, c);
        let marco =
            logica::Marco { ultima: ultima(hoja), fijas: (ab.rejilla.filas_fijas, ab.rejilla.columnas_fijas), filas_por_pagina, lleno: &lleno_en };
        let nueva = logica::mover(ab.sel, m, extender, &marco);
        // RePag/AvPag mueven tambien la vista una pantalla, como Excel.
        if matches!(m, Mov::RePag | Mov::AvPag) {
            let d = ab.rejilla.y(nueva.activa.0) - ab.rejilla.y(ab.sel.activa.0);
            ab.y += d;
        }
        ab.sel = nueva;
        asegurar_visible(ab, celdas, nueva.activa);
    }

    fn poner_zoom(&mut self, z: f64, foco: Option<(f32, f32)>) {
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        let (_, celdas) = vista_de(ab, caja);
        let z1 = logica::acotar_zoom(z);
        let (rx, ry) = foco.map_or((0.0, 0.0), |(x, y)| ((x - celdas.x) as f64, (y - celdas.y) as f64));
        let r = &ab.rejilla;
        ab.x = logica::con_foco(ab.x, rx, r.x(r.columnas_fijas), ab.zoom, z1);
        ab.y = logica::con_foco(ab.y, ry, r.y(r.filas_fijas), ab.zoom, z1);
        ab.zoom = z1;
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
    }

    fn rueda(&mut self, delta: i32, horizontal: bool) {
        let m = crate::lector::muescas(delta);
        if dentro(&self.zonas().pestanas, self.raton) {
            // Sobre la tira de pestanas, se corre la tira.
            let n = self.libro.hojas.len();
            self.desde_pestana = if m < 0.0 { (self.desde_pestana + 1).min(n.saturating_sub(1)) } else { self.desde_pestana.saturating_sub(1) };
            return;
        }
        let mods = pixpin_shell::entrada::modificadores_pulsados();
        if mods.ctrl && !horizontal {
            let z = self.abierta().zoom;
            let raton = self.raton;
            self.poner_zoom(logica::zoom_con_rueda(z, m), Some(raton));
            return;
        }
        let caja = self.zonas().hoja;
        let e = self.e as f64;
        let ab = self.abierta_mut();
        if horizontal || mods.shift {
            // Unos 120 px de pantalla por muesca; a la derecha con la rueda
            // hacia abajo (Mayus) o con el gesto hacia la derecha.
            let paso = 40.0 * e / ab.zoom;
            let signo = if horizontal { 1.0 } else { -1.0 };
            ab.x += signo * m as f64 * 3.0 * paso;
        } else {
            // Tres filas por muesca, como Excel.
            ab.y += crate::lector::desplazamiento_de_rueda(delta, ab.rejilla.alto_defecto as f32) as f64;
        }
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
    }

    /// La celda bajo el raton, acotada a la zona de celdas (para seguir
    /// eligiendo al salirse arrastrando).
    fn celda_bajo(&self, (x, y): (f32, f32), acotada: bool) -> Option<(u32, u32)> {
        let ab = self.abierta();
        let (v, celdas) = self.vista();
        let (mut px, mut py) = ((x - celdas.x) as f64, (y - celdas.y) as f64);
        if acotada {
            px = px.clamp(0.0, (celdas.ancho as f64 - 1.0).max(0.0));
            py = py.clamp(0.0, (celdas.alto as f64 - 1.0).max(0.0));
        }
        ab.rejilla.celda_en_hoja(&ab.hoja, &vista_celdas(&v, celdas), px, py)
    }

    /// Mientras se arrastra fuera de las celdas, la vista corre hacia alli.
    fn correr_al_arrastrar(&mut self) -> bool {
        if !matches!(self.gesto, Some(Gesto::Elegir { .. } | Gesto::Encabezado(_))) {
            return false;
        }
        let (_, celdas) = self.vista();
        let (x, y) = self.raton;
        let fuera = |v: f32, a: f32, b: f32| {
            if v < a {
                v - a
            } else if v > b {
                v - b
            } else {
                0.0
            }
        };
        let dx = fuera(x, celdas.x, celdas.x + celdas.ancho);
        let dy = fuera(y, celdas.y, celdas.y + celdas.alto);
        if dx == 0.0 && dy == 0.0 {
            return false;
        }
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        let z = ab.zoom;
        ab.x += (dx.clamp(-60.0, 60.0) as f64 * 0.5) / z;
        ab.y += (dy.clamp(-60.0, 60.0) as f64 * 0.5) / z;
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
        self.arrastrar_seleccion();
        true
    }

    fn arrastrar_seleccion(&mut self) {
        let raton = self.raton;
        match self.gesto {
            Some(Gesto::Elegir { .. }) => {
                if let Some(c) = self.celda_bajo(raton, true) {
                    self.abierta_mut().sel.activa = c;
                }
            }
            Some(Gesto::Encabezado(eje)) => {
                if let Some((f, c)) = self.celda_bajo(raton, true) {
                    let (uf, uc) = ultima(&self.abierta().hoja);
                    let ab = self.abierta_mut();
                    ab.sel.activa = match eje {
                        Eje::Vertical => (f, ab.sel.activa.1.max(uc)),
                        Eje::Horizontal => (ab.sel.activa.0.max(uf), c),
                    };
                }
            }
            _ => {}
        }
    }

    /// Lleva la barra `eje` a donde esta el raton (con el pulgar cogido por
    /// `agarre`).
    fn llevar_barra(&mut self, eje: Eje, agarre: f32) {
        let caja = self.zonas().hoja;
        let (e, encima, raton) = (self.e, self.encima_de, self.raton);
        let ab = self.abierta_mut();
        let (_, celdas) = vista_de(ab, caja);
        let (bv, bh) = barras(ab, celdas, e, encima);
        match (eje, bv, bh) {
            (Eje::Vertical, Some(b), _) => {
                ab.y = logica::pos_de_pulgar(b.pista.alto, b.pulgar.alto, raton.1 - agarre - b.pista.y, b.min, b.max);
            }
            (Eje::Horizontal, _, Some(b)) => {
                ab.x = logica::pos_de_pulgar(b.pista.ancho, b.pulgar.ancho, raton.0 - agarre - b.pista.x, b.min, b.max);
            }
            _ => {}
        }
    }

    /// **Un clic** en la ventana. `true` si empieza un arrastre (hay que
    /// capturar el raton).
    fn pulsar(&mut self, motor: &MotorRender) -> bool {
        let p = self.raton;
        let z = self.zonas();
        // Las pestanas.
        if dentro(&z.pestanas, p) {
            if let Some(t) = self.pestanas.iter().find(|t| dentro(&t.caja, p)) {
                let i = t.hoja;
                self.cambiar_hoja(i, motor);
            }
            return false;
        }
        if !dentro(&z.hoja, p) {
            return false;
        }
        // Las barras.
        let (_, celdas) = self.vista();
        let (bv, bh) = barras(self.abierta(), celdas, self.e, self.encima_de);
        for (eje, b) in [(Eje::Vertical, bv), (Eje::Horizontal, bh)] {
            let Some(b) = b else { continue };
            if !dentro(&zona_de_barra(&b, eje, self.e), p) {
                continue;
            }
            let (en, desde, largo) = match eje {
                Eje::Vertical => (p.1, b.pulgar.y, b.pulgar.alto),
                Eje::Horizontal => (p.0, b.pulgar.x, b.pulgar.ancho),
            };
            // En el pulgar se coge donde se pulso; en la pista, por el centro.
            let agarre = if en >= desde && en < desde + largo { en - desde } else { largo / 2.0 };
            self.gesto = Some(Gesto::Barra { eje, agarre });
            self.encima_de = Some(eje);
            self.llevar_barra(eje, agarre);
            return true;
        }
        let mods = pixpin_shell::entrada::modificadores_pulsados();
        let (uf, uc) = ultima(&self.abierta().hoja);
        // Los encabezados: filas o columnas enteras, la esquina todo.
        let en_numeros = p.0 < celdas.x;
        let en_letras = p.1 < celdas.y;
        if en_numeros || en_letras {
            let celda = self.celda_bajo(p, true);
            let ab = self.abierta_mut();
            let (eje, sel) = match (en_numeros, en_letras, celda) {
                (true, true, _) => {
                    ab.sel = Seleccion { ancla: (0, 0), activa: (uf, uc) };
                    return false;
                }
                (true, false, Some((f, _))) => (Eje::Vertical, Seleccion { ancla: (f, 0), activa: (f, uc) }),
                (false, true, Some((_, c))) => (Eje::Horizontal, Seleccion { ancla: (0, c), activa: (uf, c) }),
                _ => return false,
            };
            ab.sel = if mods.shift { Seleccion { ancla: ab.sel.ancla, activa: sel.activa } } else { sel };
            self.gesto = Some(Gesto::Encabezado(eje));
            return true;
        }
        let Some(celda) = self.celda_bajo(p, false) else { return false };
        let ab = self.abierta_mut();
        ab.sel = if mods.shift { Seleccion { ancla: ab.sel.ancla, activa: celda } } else { Seleccion::en(celda.0, celda.1) };
        self.gesto = Some(Gesto::Elegir { pulsada: celda, cuando: Instant::now(), ctrl: mods.ctrl });
        true
    }

    /// Al soltar: un clic corto en una celda con enlace lo abre.
    fn soltar(&mut self, textos: &Catalogo, motor: &MotorRender) {
        let gesto = self.gesto.take();
        if let Some(Gesto::Elegir { pulsada, cuando, .. }) = gesto {
            let sel = self.abierta().sel;
            if sel.ancla == pulsada && sel.activa == pulsada && cuando.elapsed() < CLIC_DE_ENLACE {
                self.abrir_enlace(pulsada, textos, motor);
            }
        }
    }

    /// El cursor que toca: la mano sobre un enlace.
    fn cursor(&self) -> FormaCursorWin {
        if self.gesto.is_none()
            && dentro(&self.vista().1, self.raton)
            && let Some((f, c)) = self.celda_bajo(self.raton, false)
            && self.abierta().hoja.hipervinculo_en(f, c).is_some()
        {
            // `Giro` es IDC_HAND (ver su nota en `FormaCursorWin`).
            return FormaCursorWin::Giro;
        }
        FormaCursorWin::Flecha
    }

    /// Que barra tiene el raton encima.
    fn barra_bajo_el_raton(&self) -> Option<Eje> {
        if let Some(Gesto::Barra { eje, .. }) = self.gesto {
            return Some(eje);
        }
        let (_, celdas) = self.vista();
        let (bv, bh) = barras(self.abierta(), celdas, self.e, None);
        if bv.is_some_and(|b| dentro(&zona_de_barra(&b, Eje::Vertical, self.e), self.raton)) {
            return Some(Eje::Vertical);
        }
        if bh.is_some_and(|b| dentro(&zona_de_barra(&b, Eje::Horizontal, self.e), self.raton)) {
            return Some(Eje::Horizontal);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// La ventana

/// La escala del monitor donde cae el punto (o la del principal).
fn escala_en(p: Punto) -> u32 {
    let Ok(d) = pixpin_capture::enumerar_monitores() else { return 100 };
    d.monitores()
        .iter()
        .find(|m| p.x >= m.area.x && p.y >= m.area.y && p.x < m.area.x + m.area.ancho as i32 && p.y < m.area.y + m.area.alto as i32)
        .or_else(|| d.principal())
        .map_or(100, |m| m.escala_por_cien)
}

/// **La ventana del pin de la hoja**: la abre y alterna entre ver (aqui) y
/// editar (`hoja_pin::editar`, si la hoja es del chat) sin cerrarla.
#[allow(clippy::too_many_arguments)] // lo que se abre y de donde viene
fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    libro: &mut Libro,
    ruta: Option<&Path>,
    nombre: &str,
    t0: Instant,
    mut editable: Option<&mut crate::hoja_pin::Editable>,
) -> Result<()> {
    // Cuenta como abierta para los grupos de ventanas (H9) mientras viva.
    let _grupo = ruta.map(|r| crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lector { ruta: r.to_path_buf() }));
    if libro.hojas.is_empty() {
        anyhow::bail!("el libro no tiene hojas");
    }
    let (area_pin, _) = crate::lector_pin::area_inicial();
    let escala = escala_en(Punto { x: area_pin.x + area_pin.ancho as i32 / 2, y: area_pin.y + area_pin.alto as i32 / 2 });
    let e = escala as f32 / 100.0;
    let pin = PinLector::nuevo(area_pin, e);
    let area = pin.area;
    let ventana = VentanaOverlay::nueva_normal(area, nombre).context("no se pudo abrir la ventana del visor de hojas")?;
    let motor = recursos.motor();
    let superficie =
        Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), area.ancho, area.alto).context("sin superficie para el visor de hojas")?;
    ventana.mostrar();
    ventana.enfocar();
    // Como un pin: siempre encima (se suelta con su pin) y su barra fuera.
    ventana.poner_siempre_encima(true);
    let mut marco = Marco { ventana, superficie, pin, area, e, nombre: nombre.to_string() };
    marco.pin.crear_barra(&motor, &recursos.d3d(), &marco.ventana);
    // Desde cuando se cuenta la primera pantalla de cada vuelta a ver.
    let mut desde = t0;
    let mut activa = libro.hoja_activa.min(libro.hojas.len().saturating_sub(1));
    // Una tabla recien creada se abre ya editando.
    let mut empezar = editable.as_ref().is_some_and(|e| e.al_abrir_editar).then_some(Fin::Editar(activa));
    loop {
        let fin = match empezar.take() {
            Some(f) => f,
            None => {
                let (m, fin) = ver_en(marco, &motor, textos, libro, activa, ruta, editable.is_some(), desde);
                marco = m;
                fin
            }
        };
        let Fin::Editar(vista) = fin else { break };
        activa = vista;
        let Some(ed) = editable.as_deref_mut() else { continue };
        match crate::hoja_pin::editar(&mut marco, &motor, textos, ed, libro, vista) {
            crate::hoja_pin::Vuelta::Cerrar => break,
            crate::hoja_pin::Vuelta::Ver { activa: a, libro: nuevo } => {
                if let Some(n) = nuevo {
                    *libro = n;
                }
                activa = a.min(libro.hojas.len().saturating_sub(1));
                desde = Instant::now();
            }
        }
    }
    Ok(())
}

/// **El modo ver**: el bucle de la ventana con el libro hasta que se cierra
/// o se pulsa «Editar». Devuelve la ventana para seguir con ella.
#[allow(clippy::too_many_arguments)] // la ventana y lo que se ve en ella
fn ver_en(
    marco: Marco,
    motor: &MotorRender,
    textos: &Catalogo,
    libro: &Libro,
    activa: usize,
    ruta: Option<&Path>,
    editable: bool,
    t0: Instant,
) -> (Marco, Fin) {
    let Marco { mut ventana, superficie, mut pin, mut area, e, nombre } = marco;
    let mut fin = Fin::Cerrar;
    let mut est = Estado {
        libro,
        hojas: (0..libro.hojas.len()).map(|_| None).collect(),
        activa: activa.min(libro.hojas.len().saturating_sub(1)),
        cache: CacheHoja::nueva(),
        digito: None,
        ppp: 96.0 * e as f64,
        e,
        ancho: area.ancho as f32,
        alto: area.alto as f32,
        pestanas: Vec::new(),
        desde_pestana: 0,
        gesto: None,
        raton: (0.0, 0.0),
        encima_de: None,
        aviso: None,
        hwnd: ventana.handle().0 as isize,
    };
    let activa = est.activa;
    est.abrir_hoja(activa, motor);
    est.desde_pestana = activa.saturating_sub(3);

    let botones = botones_de_ver(editable, ruta.is_some());
    let mut hay_que_pintar = true;
    let mut primera = true;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        // Las ordenes de las pruebas del pin de hoja (`hoja_pin::prueba`).
        #[cfg(test)]
        match crate::hoja_pin::prueba::latido(ventana.handle().0 as isize, area, false) {
            Some(crate::hoja_pin::prueba::EDITAR) => {
                fin = Fin::Editar(est.activa);
                vivo = false;
            }
            Some(crate::hoja_pin::prueba::CERRAR) => vivo = false,
            _ => {}
        }
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if pin.es_de_la_barra(hwnd) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(AccionPin::Propio(BOTON_COPIAR)) => est.copiar(textos),
                    Some(AccionPin::Propio(BOTON_SU_PROGRAMA)) => {
                        if let Some(ruta) = ruta
                            && let Err(e) = pixpin_shell::abrir(ruta)
                        {
                            tracing::warn!(?e, "no se pudo abrir el libro con su programa");
                        }
                    }
                    Some(AccionPin::Propio(BOTON_EDITAR)) => {
                        fin = Fin::Editar(est.activa);
                        vivo = false;
                    }
                    Some(AccionPin::PantallaCompleta) => pin.alternar_pantalla_completa(&mut ventana),
                    Some(AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(AccionPin::Cerrar) => vivo = false,
                    _ => {}
                }
                hay_que_pintar = true;
                continue;
            }
            if hwnd != ventana.handle() {
                continue;
            }
            // Estirar por los bordes es del pin.
            if pin.evento_ventana(&evento, &mut ventana).consumido {
                hay_que_pintar = true;
                continue;
            }
            let local = |p: Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Despierta => {}
                EventoOverlay::RatonMovido(p) => {
                    est.raton = local(p);
                    match est.gesto {
                        Some(Gesto::Barra { eje, agarre }) => {
                            est.llevar_barra(eje, agarre);
                            hay_que_pintar = true;
                        }
                        Some(_) => {
                            let antes = est.abierta().sel;
                            est.arrastrar_seleccion();
                            hay_que_pintar |= est.abierta().sel != antes;
                        }
                        None => {
                            let encima = est.barra_bajo_el_raton();
                            if encima != est.encima_de {
                                est.encima_de = encima;
                                hay_que_pintar = true;
                            }
                        }
                    }
                    ventana.poner_cursor(est.cursor());
                }
                EventoOverlay::BotonPulsado(p) => {
                    est.raton = local(p);
                    let ctrl = pixpin_shell::entrada::modificadores_pulsados().ctrl;
                    if est.pulsar(&motor) {
                        ventana.capturar_raton();
                    }
                    // Ctrl+clic en un enlace lo abre ya.
                    if ctrl
                        && let Some(Gesto::Elegir { pulsada, .. }) = est.gesto
                        && est.abrir_enlace(pulsada, textos, &motor)
                    {
                        est.gesto = None;
                        ventana.soltar_raton();
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::BotonSoltado(p) => {
                    est.raton = local(p);
                    est.soltar(textos, &motor);
                    est.encima_de = est.barra_bajo_el_raton();
                    ventana.soltar_raton();
                    hay_que_pintar = true;
                }
                EventoOverlay::Rueda(delta) => {
                    est.rueda(delta, false);
                    hay_que_pintar = true;
                }
                EventoOverlay::RuedaHorizontal(delta) => {
                    est.rueda(delta, true);
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, ctrl, alt, .. } if editable && pide_editar(vk, ctrl, alt) => {
                    fin = Fin::Editar(est.activa);
                    vivo = false;
                }
                EventoOverlay::Tecla { vk, shift, ctrl, alt } => {
                    match logica::accion_de_tecla(vk, shift, ctrl, alt) {
                        Some(AccionTecla::Mover(m, ext)) => est.mover(m, ext),
                        Some(AccionTecla::HojaAnterior) => {
                            if let Some(i) = est.hoja_vecina(false) {
                                est.cambiar_hoja(i, &motor);
                            }
                        }
                        Some(AccionTecla::HojaSiguiente) => {
                            if let Some(i) = est.hoja_vecina(true) {
                                est.cambiar_hoja(i, &motor);
                            }
                        }
                        Some(AccionTecla::Copiar) => est.copiar(textos),
                        Some(AccionTecla::SeleccionarTodo) => {
                            let u = ultima(&est.abierta().hoja);
                            est.abierta_mut().sel = Seleccion { ancla: (0, 0), activa: u };
                        }
                        Some(AccionTecla::ZoomInicial) => est.poner_zoom(1.0, None),
                        Some(AccionTecla::Acercar) => {
                            let z = est.abierta().zoom;
                            est.poner_zoom(logica::zoom_con_rueda(z, 1.0), None);
                        }
                        Some(AccionTecla::Alejar) => {
                            let z = est.abierta().zoom;
                            est.poner_zoom(logica::zoom_con_rueda(z, -1.0), None);
                        }
                        Some(AccionTecla::PantallaCompleta) => pin.alternar_pantalla_completa(&mut ventana),
                        Some(AccionTecla::AbrirEnlace) => {
                            let a = est.abierta().sel.activa;
                            est.abrir_enlace(a, textos, &motor);
                        }
                        Some(AccionTecla::Escape) => {
                            if pin.pantalla_completa {
                                pin.alternar_pantalla_completa(&mut ventana);
                            } else {
                                vivo = false;
                            }
                        }
                        None => {}
                    }
                    hay_que_pintar = true;
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if est.recibir() {
            hay_que_pintar = true;
        }
        if est.correr_al_arrastrar() {
            hay_que_pintar = true;
        }
        // Movida o estirada desde fuera (AltSnap, Win+flecha): el pin la sigue.
        pin.seguir_ventana(&mut ventana);
        if pin.area != area {
            if (pin.area.ancho, pin.area.alto) != (area.ancho, area.alto) {
                let _ = superficie.redimensionar(pin.area.ancho, pin.area.alto);
            }
            area = pin.area;
            est.ancho = area.ancho as f32;
            est.alto = area.alto as f32;
            let caja = est.zonas().hoja;
            let ab = est.abierta_mut();
            let (_, celdas) = vista_de(ab, caja);
            acotar(ab, celdas);
            hay_que_pintar = true;
        }
        pin.actualizar(&motor, &nombre, &botones);
        if est.aviso.as_ref().is_some_and(|(_, hasta)| Instant::now() >= *hasta) {
            est.aviso = None;
            hay_que_pintar = true;
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            preparar(&mut est, &motor);
            if let Ok(destino) = superficie.empezar(&motor) {
                let t = Instant::now();
                let _ = motor.dibujar(&destino, |p: &Pintor| pintar(p, &mut est, textos));
                let _ = superficie.presentar();
                if primera {
                    primera = false;
                    tracing::info!(ms = ms(t0), pintar_ms = ms(t), "visor de hojas: primera pantalla pintada");
                }
            }
        }
        let tope = if est.gesto.is_some() {
            16
        } else if est.cargando() || pin.quiere_reloj() || est.aviso.is_some() {
            50
        } else {
            200
        };
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }
    (Marco { ventana, superficie, pin, area, e, nombre }, fin)
}

/// Los botones de la barra en modo ver: «Editar» si la hoja es del chat,
/// copiar y, si hay archivo, abrirlo con su programa.
fn botones_de_ver(editable: bool, con_archivo: bool) -> Vec<BotonLector> {
    let mut b = Vec::with_capacity(3);
    if editable {
        b.push(BotonLector { id: BOTON_EDITAR, icono: &material::EDIT, encendido: false });
    }
    b.push(BotonLector { id: BOTON_COPIAR, icono: &material::CONTENT_COPY, encendido: false });
    if con_archivo {
        b.push(BotonLector { id: BOTON_SU_PROGRAMA, icono: &material::OPEN_IN_NEW, encendido: false });
    }
    b
}

/// F2 o la `E` sola (sin Ctrl ni Alt) pasan a editar.
fn pide_editar(vk: u32, ctrl: bool, alt: bool) -> bool {
    !ctrl && !alt && (vk == VK_F2 || vk == VK_E)
}

// ---------------------------------------------------------------------------
// Pintar

/// Antes de pintar: el alto de las filas que se van a ver sin alto propio
/// (texto ajustado), como hace Excel al abrir.
fn preparar(est: &mut Estado, motor: &MotorRender) {
    let caja = est.zonas().hoja;
    let libro = est.libro;
    let Estado { hojas, activa, cache, .. } = est;
    let Some(ab) = hojas[*activa].as_mut() else { return };
    let (v, celdas) = vista_de(ab, caja);
    let paneles = ab.rejilla.paneles(&vista_celdas(&v, celdas));
    for p in paneles {
        hoja_office::autoajustar_filas(cache, motor, libro, &ab.hoja, &mut ab.rejilla, p.fila0, p.fila1 + 1);
    }
    let (_, celdas) = vista_de(ab, caja);
    acotar(ab, celdas);
}

fn texto_centrado_v(p: &Pintor, texto: &str, x: f32, caja: RectF, tam: f32, ancho: f32, color: Color) {
    let (_, h) = p.medir_texto("Ag", tam);
    p.texto_linea(texto, x, caja.y + (caja.alto - h) / 2.0, tam, ancho.max(0.0), color);
}

fn pintar(p: &Pintor, est: &mut Estado, textos: &Catalogo) {
    let z = est.zonas();
    let (e, libro, ancho) = (est.e, est.libro, est.ancho);
    p.limpiar(Color::BLANCO);
    let Estado { hojas, activa, cache, desde_pestana, encima_de, pestanas, aviso, .. } = est;
    let activa = *activa;
    let Some(ab) = hojas[activa].as_ref() else { return };

    // La hoja.
    let (v, celdas) = vista_de(ab, z.hoja);
    let op = OpcionesHoja { encabezados: true, seleccion: Some(ab.sel.rango()) };
    hoja_office::pintar_hoja(p, cache, libro, &ab.hoja, &ab.rejilla, &v, z.hoja, &op);

    // Las barras de desplazamiento.
    let (bv, bh) = barras(ab, celdas, e, *encima_de);
    for (eje, b) in [(Eje::Vertical, bv), (Eje::Horizontal, bh)] {
        if let Some(b) = b {
            let color = if *encima_de == Some(eje) { PULGAR_ENCIMA } else { PULGAR };
            let r = b.pulgar.ancho.min(b.pulgar.alto) / 2.0;
            p.rellenar_redondeado(b.pulgar, r, color);
        }
    }

    // La barra de formulas: la celda activa y lo que tiene.
    let f = z.formula;
    p.rellenar(f, FONDO_BARRA);
    p.rellenar(RectF { y: f.y + f.alto - 1.0, alto: 1.0, ..f }, LINEA);
    let tam = TAM_TEXTO * e;
    let (fa, ca) = ab.sel.activa;
    let r = ab.sel.rango();
    let referencia = if r.fila0 == r.fila1 && r.col0 == r.col1 {
        format!("{}{}", nombre_columna(ca), fa + 1)
    } else {
        format!("{}{}:{}{}", nombre_columna(r.col0), r.fila0 + 1, nombre_columna(r.col1), r.fila1 + 1)
    };
    let ancho_ref = 150.0 * e;
    texto_centrado_v(p, &referencia, f.x + 8.0 * e, f, tam, ancho_ref - 12.0 * e, TEXTO);
    p.rellenar(RectF { x: f.x + ancho_ref, y: f.y + 5.0 * e, ancho: 1.0, alto: f.alto - 10.0 * e }, LINEA);
    let celda = ab.hoja.celda(fa, ca);
    let contenido = celda.and_then(|c| libro.contenido(&ab.hoja, c)).map(|c| c.mostrado.texto).unwrap_or_default();
    let x0 = f.x + ancho_ref + 10.0 * e;
    let enlace = ab.hoja.hipervinculo_en(fa, ca).map(|h| h.destino.clone());
    let ancho_enlace = if enlace.is_some() { (f.ancho * 0.35).min(360.0 * e) } else { 0.0 };
    let una_linea = contenido.replace(['\r', '\n'], " ⏎ ");
    texto_centrado_v(p, &una_linea, x0, f, tam, f.x + f.ancho - x0 - ancho_enlace - 10.0 * e, TEXTO);
    if let Some(d) = enlace {
        texto_centrado_v(p, &d, f.x + f.ancho - ancho_enlace - 6.0 * e, f, tam, ancho_enlace, ENLACE);
    }

    // Las pestanas y, a la derecha, lo que falta por leer y el zoom.
    let mut estado = Vec::new();
    if !ab.hoja.completa {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("filas", ab.hoja.filas.len());
        estado.push(textos.t_args("hoja-cargando", &args));
    }
    if (ab.zoom - 1.0).abs() > 0.004 {
        estado.push(format!("{:.0} %", ab.zoom * 100.0));
    }
    let estado = estado.join("   ·   ");
    let ancho_estado = if estado.is_empty() { 0.0 } else { p.medir_texto(&estado, tam).0 + 24.0 * e };
    let tira = RectF { ancho: (z.pestanas.ancho - ancho_estado).max(0.0), ..z.pestanas };
    *pestanas = hoja_office::pintar_pestanas(p, cache, libro, activa, *desde_pestana, tira, e);
    if ancho_estado > 0.0 {
        let caja = RectF { x: z.pestanas.x + z.pestanas.ancho - ancho_estado, ancho: ancho_estado, ..z.pestanas };
        p.rellenar(caja, Color { r: 0.953, g: 0.953, b: 0.953, a: 1.0 });
        p.rellenar(RectF { alto: 1.0, ..caja }, LINEA);
        texto_centrado_v(p, &estado, caja.x + 12.0 * e, caja, tam, ancho_estado - 16.0 * e, TENUE);
    }

    if let Some((texto, _)) = aviso {
        crate::lector::aviso_en(p, texto, ancho / 2.0, z.pestanas.y - 56.0 * e, e);
    }
}

#[cfg(test)]
mod pruebas;
