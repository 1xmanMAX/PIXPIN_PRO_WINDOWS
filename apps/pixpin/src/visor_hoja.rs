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
//! que se ve en cada celda; no hay boton «Copiar»: el usuario, 12-oct-2026,
//! «Ctrl+C ya copia siempre»); un clic en un enlace lo abre (web o un sitio
//! del libro); las pestanas de abajo o Ctrl+RePag/AvPag cambian de hoja.
//! Arriba, una linea como la barra de formulas: la celda activa y lo que
//! tiene.
//!
//! **Una hoja del chat se edita en este mismo visor** (`hoja_pin`): la
//! misma ventana, el mismo pintor y los mismos gestos; editar solo anade
//! escribir en las casillas, las formulas y la barra de formato. Pasar de
//! ver a editar y de vuelta conserva el zoom, la posicion y la celda activa.
//!
//! Un libro que no se puede leer (un `.xls` renombrado, uno roto, uno
//! binario) se abre con su programa, como antes.

mod capa;
mod logica;
mod marcos;
mod riel;

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Punto;
use pixpin_hojas::{
    Hoja, LectorHoja, Libro, Rango, Regional, Rejilla, Tanda, Valor, Visibilidad, Vista,
    nombre_columna,
};
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

const FONDO_BARRA: Color = Color {
    r: 0.953,
    g: 0.953,
    b: 0.953,
    a: 1.0,
};
const LINEA: Color = Color {
    r: 0.831,
    g: 0.831,
    b: 0.831,
    a: 1.0,
};
const TEXTO: Color = Color {
    r: 0.13,
    g: 0.13,
    b: 0.13,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.42,
    g: 0.42,
    b: 0.42,
    a: 1.0,
};
const ENLACE: Color = Color {
    r: 0.02,
    g: 0.39,
    b: 0.76,
    a: 1.0,
};
const PULGAR: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.28,
};
const PULGAR_ENCIMA: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.45,
};
/// El verde de Excel (lo que dice que se esta editando).
const VERDE: Color = Color {
    r: 0.129,
    g: 0.451,
    b: 0.275,
    a: 1.0,
};
const AVISO_LARGO: Duration = Duration::from_millis(5000);
/// Dos clics en la misma celda antes de esto, editando: escribir en ella.
const DOBLE_CLIC: Duration = Duration::from_millis(450);
/// Lo que se ignoran las letras al pasar a editar con una tecla (la `E` que
/// trajo hasta aqui llega despues como caracter y no se escribe).
const SIN_LETRAS: Duration = Duration::from_millis(250);

/// Botones propios en la barra del pin (los de editar, en `hoja_pin`).
const BOTON_ABRIR_CON: u8 = 2;
/// «Editar»: el mismo visor pasa a editar (`hoja_pin`). Solo en las hojas
/// del chat, que es donde se guarda lo editado.
const BOTON_EDITAR: u8 = 3;
/// F2, como en Excel, y la `E` de «Editar».
const VK_F2: u32 = 0x71;
const VK_E: u32 = 0x45;
const VK_Y: u32 = 0x59;
const VK_Z: u32 = 0x5A;
/// La A del lapiz y la M del marcador (o Ctrl+M), como en el PDF.
const VK_A: u32 = 0x41;
const VK_M: u32 = 0x4D;

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
    let lanzado = std::thread::Builder::new()
        .name("visor-hoja".into())
        .spawn(move || ver_editable(idioma, editable, t0));
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
    tracing::info!(
        ms = ms(t0),
        hojas = libro.hojas.len(),
        "visor de hojas: hoja del chat abierta"
    );
    let textos = Catalogo::nuevo(idioma);
    let ruta = ed.ruta_original();
    let nombre = ed.nombre();
    let hecho = crate::dispositivo_perdido::con_recursos("visor-hoja", |r| {
        abrir(
            r,
            &textos,
            &mut libro,
            ruta.as_deref(),
            &nombre,
            t0,
            Some(&mut ed),
        )
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
    let lanzado = std::thread::Builder::new()
        .name("visor-hoja".into())
        .spawn(move || ver(idioma, &ruta, t0));
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
    let hecho = crate::dispositivo_perdido::con_recursos("visor-hoja", |r| {
        abrir(r, &textos, &mut libro, Some(ruta), &nombre, t0, None)
    });
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
    Elegir {
        pulsada: (u32, u32),
        cuando: Instant,
        ctrl: bool,
    },
    /// Eligiendo filas o columnas enteras por los encabezados.
    Encabezado(Eje),
    /// Llevando el pulgar de una barra; `agarre`: donde se cogio, desde su
    /// principio.
    Barra { eje: Eje, agarre: f32 },
    /// Moviendo la hoja con el boton central (la rueda) pulsado, como el
    /// lienzo y el lector: `desde`, donde se pulso; `x`, `y`, la vista de
    /// entonces. No elige celdas.
    Mano { desde: (f32, f32), x: f64, y: f64 },
    /// Arrastrando un marco para imprimir (`marcos`).
    Marco,
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

/// Donde se estaba mirando una hoja: se conserva al rehacer lo que se ve
/// tras un cambio (editando), para que no salte nada.
#[derive(Debug, Clone, Copy)]
struct Posicion {
    x: f64,
    y: f64,
    zoom: f64,
    sel: Seleccion,
}

impl Abierta {
    fn posicion(&self) -> Posicion {
        Posicion {
            x: self.x,
            y: self.y,
            zoom: self.zoom,
            sel: self.sel,
        }
    }
}

struct Estado<'l> {
    /// Lo que se ve. Editando, se rehace con cada cambio (`hoja_pin`).
    libro: &'l mut Libro,
    /// La hoja del chat, si lo es (se puede editar).
    ed: Option<&'l mut crate::hoja_pin::Editable>,
    /// Editando (la barra de formato, escribir en las casillas).
    editando: bool,
    /// El ultimo clic en una celda, editando (dos seguidos: escribir).
    ultimo_clic: Option<((u32, u32), Instant)>,
    /// Hasta cuando no se escriben las letras que llegan.
    sin_letras_hasta: Option<Instant>,
    /// Cuantas veces se rehizo lo que se ve (para que el pintor no confunda
    /// la hoja nueva con la de antes, que vive en la misma direccion).
    generacion: u64,
    /// Donde se miraban las hojas que se cerraron al rehacer lo que se ve.
    guardadas: std::collections::HashMap<usize, Posicion>,
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
    /// La celda bajo el raton (lleva el boton de su lista desplegable).
    bajo_raton: Option<(u32, u32)>,
    /// La celda con comentario bajo el raton (sale su tarjeta).
    comentario_bajo: Option<(u32, u32)>,
    aviso: Option<(String, Instant)>,
    hwnd: isize,
    /// Lo que anoto el asistente (o el lapiz del riel) en la hoja abierta
    /// (`capa`).
    anotado: capa::Anotado,
    /// El nombre de la ventana (el titulo al imprimir con el marco).
    nombre: String,
    /// Donde esta la ventana en el escritorio (la tinta cuenta desde ahi).
    area: pixpin_geom::Rect,
    /// **El riel** (`riel`): anotando con el lapiz, poniendo una marca, lo
    /// que tiene el raton encima y las marcas del libro.
    anotando: bool,
    poniendo_marca: bool,
    encima: pixpin_ui::riel_marcas::DestinoRiel,
    marcas: Vec<pixpin_motor2d::marcas::Marca>,
    recuerdo: riel::Recuerdo,
    /// La tinta cambio algo en el gesto en curso (se guarda al acabarlo).
    tinta_cambio: bool,
    /// Un trazo con el raton capturado.
    trazo: bool,
    /// **Los marcos numerados para imprimir** (`marcos`, como los del plano)
    /// y lo que guarda cada marca del riel ademas de su celda: el zoom y el
    /// desplazamiento exactos (`marcos_doc::Vistas`).
    marcos: crate::marcos_doc::Marcos,
    vistas: crate::marcos_doc::Vistas,
    /// Pintando para la foto de un marco: sin el riel, los marcos, lo
    /// elegido ni las barras.
    sin_interfaz: bool,
}

fn zonas(ancho: f32, alto: f32, e: f32) -> Zonas {
    let (af, ap) = ((ALTO_FORMULA * e).round(), (ALTO_PESTANAS * e).round());
    Zonas {
        formula: RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto: af,
        },
        hoja: RectF {
            x: 0.0,
            y: af,
            ancho,
            alto: (alto - af - ap).max(0.0),
        },
        pestanas: RectF {
            x: 0.0,
            y: (alto - ap).max(af),
            ancho,
            alto: ap,
        },
    }
}

fn dentro(r: &RectF, (x, y): (f32, f32)) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.ancho && y < r.y + r.alto
}

/// La vista con que se pinta la hoja en `caja` y la zona de sus celdas
/// (sin encabezados).
fn vista_de(ab: &Abierta, caja: RectF) -> (Vista, RectF) {
    let v = Vista {
        ancho: caja.ancho as f64,
        alto: caja.alto as f64,
        escala: ab.zoom,
        x: ab.x,
        y: ab.y,
    };
    let celdas = hoja_office::zona_de_celdas(&ab.hoja, &ab.rejilla, &v, caja, true);
    (v, celdas)
}

/// La vista de las cuentas de la rejilla: del tamano de las celdas.
fn vista_celdas(v: &Vista, celdas: RectF) -> Vista {
    Vista {
        ancho: celdas.ancho as f64,
        alto: celdas.alto as f64,
        ..*v
    }
}

/// La ultima fila y columna usadas (con la `dimension` mientras se lee).
fn ultima(h: &Hoja) -> (u32, u32) {
    let d = h.cabecera.dimension;
    (
        h.max_fila().max(d.map_or(0, |d| d.fila1)),
        h.max_col.max(d.map_or(0, |d| d.col1)),
    )
}

fn lleno(h: &Hoja, f: u32, c: u32) -> bool {
    h.celda(f, c).is_some_and(|c| c.valor != Valor::Vacio)
}

/// Lo que se ve (px de hoja) de la zona que se desplaza, en cada eje.
fn visible(ab: &Abierta, celdas: RectF) -> (f64, f64) {
    let z = ab.zoom.max(0.01);
    let fx = ab.rejilla.x(ab.rejilla.columnas_fijas);
    let fy = ab.rejilla.y(ab.rejilla.filas_fijas);
    (
        (celdas.ancho as f64 / z - fx).max(0.0),
        (celdas.alto as f64 / z - fy).max(0.0),
    )
}

/// Hasta donde se desplaza cada eje: ((min x, max x), (min y, max y)).
fn limites_de(ab: &Abierta, celdas: RectF) -> ((f64, f64), (f64, f64)) {
    let (w, h) = ab.rejilla.tamano_usado(&ab.hoja);
    let r = &ab.rejilla;
    let lx = logica::limites(
        r.x(r.columnas_fijas),
        w,
        celdas.ancho as f64,
        ab.zoom,
        2.0 * r.ancho_defecto,
    );
    let ly = logica::limites(
        r.y(r.filas_fijas),
        h,
        celdas.alto as f64,
        ab.zoom,
        4.0 * r.alto_defecto,
    );
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
fn barras(
    ab: &Abierta,
    celdas: RectF,
    e: f32,
    encima: Option<Eje>,
) -> (Option<Barra>, Option<Barra>) {
    let ((x0, x1), (y0, y1)) = limites_de(ab, celdas);
    let (vx, vy) = visible(ab, celdas);
    let hay_v = y1 > y0;
    let hay_h = x1 > x0;
    let grosor = |eje| {
        if encima == Some(eje) {
            BARRA_ENCIMA * e
        } else {
            BARRA * e
        }
    };
    let margen = 2.0 * e;
    let (gv, gh) = (grosor(Eje::Vertical), grosor(Eje::Horizontal));
    let v = hay_v.then(|| {
        let pista = RectF {
            x: celdas.x + celdas.ancho - gv - margen,
            y: celdas.y + margen,
            ancho: gv,
            alto: (celdas.alto - 2.0 * margen - if hay_h { BARRA_ENCIMA * e } else { 0.0 })
                .max(0.0),
        };
        let (d, l) = logica::pulgar(pista.alto, ab.y, y0, y1, vy, PULGAR_MINIMO * e);
        Barra {
            pista,
            pulgar: RectF {
                y: pista.y + d,
                alto: l,
                ..pista
            },
            min: y0,
            max: y1,
        }
    });
    let h = hay_h.then(|| {
        let pista = RectF {
            x: celdas.x + margen,
            y: celdas.y + celdas.alto - gh - margen,
            ancho: (celdas.ancho - 2.0 * margen - if hay_v { BARRA_ENCIMA * e } else { 0.0 })
                .max(0.0),
            alto: gh,
        };
        let (d, l) = logica::pulgar(pista.ancho, ab.x, x0, x1, vx, PULGAR_MINIMO * e);
        Barra {
            pista,
            pulgar: RectF {
                x: pista.x + d,
                ancho: l,
                ..pista
            },
            min: x0,
            max: x1,
        }
    });
    (v, h)
}

/// Donde se puede coger una barra: un poco mas ancha que lo pintado.
fn zona_de_barra(b: &Barra, eje: Eje, e: f32) -> RectF {
    let extra = 6.0 * e;
    match eje {
        Eje::Vertical => RectF {
            x: b.pista.x - extra,
            ancho: b.pista.ancho + extra + 2.0 * e,
            ..b.pista
        },
        Eje::Horizontal => RectF {
            y: b.pista.y - extra,
            alto: b.pista.alto + extra + 2.0 * e,
            ..b.pista
        },
    }
}

/// Lee el resto de la hoja en otro hilo y manda las tandas; cada una
/// despierta a la ventana.
fn leer_aparte(mut lector: LectorHoja, hwnd: isize) -> Receiver<Tanda> {
    let (tx, rx) = std::sync::mpsc::channel();
    let lanzado = std::thread::Builder::new()
        .name("visor-hoja-leer".into())
        .spawn(move || {
            let mut vacias = 0;
            loop {
                let mut t = lector.leer(u32::MAX, TANDA_CELDAS);
                let vacia = t.filas.is_empty()
                    && t.celdas.is_empty()
                    && t.cola.is_none()
                    && t.cabecera.is_none();
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
        self.hojas[self.activa]
            .as_ref()
            .expect("la hoja activa esta abierta")
    }

    fn abierta_mut(&mut self) -> &mut Abierta {
        self.hojas[self.activa]
            .as_mut()
            .expect("la hoja activa esta abierta")
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
        if self.digito.is_none() {
            self.digito =
                self.cache
                    .ancho_digito(motor, &self.libro.estilos.fuente_normal, self.ppp);
        }
        self.abrir_hoja_con_digito(i);
    }

    /// Lo de [`Estado::abrir_hoja`] con el ancho del digito ya medido (o sin
    /// el: el de la letra por defecto).
    fn abrir_hoja_con_digito(&mut self, i: usize) {
        if self.hojas.get(i).is_none_or(|h| h.is_some()) {
            return;
        }
        let t = Instant::now();
        let libro = &*self.libro;
        let mut hoja = libro.abrir_hoja(i);
        // Rehecha tras un cambio: otra para el pintor aunque viva en el mismo
        // sitio que la de antes.
        hoja.version = hoja.version.wrapping_add(self.generacion << 32);
        // Donde se estaba mirando antes de rehacerla, si se rehizo.
        let antes = self.guardadas.remove(&i);
        // La cabecera (paneles, zoom, primera celda) y la primera fila.
        hoja.cargar_hasta_fila(0);
        let vh = hoja.vista().clone();
        let zoom = antes.map_or_else(|| logica::zoom_guardado(vh.zoom), |p| p.zoom);
        // Las filas que caben, contando con filas bajas (12 px al 100 %).
        let alto_hoja = self.zonas().hoja.alto as f64;
        let fila_baja = 12.0 * self.ppp / 96.0;
        let caben = (alto_hoja / (fila_baja * zoom)).ceil() as u32 + 2;
        let desde = antes.map_or(vh.primera_fila.max(vh.filas_fijas), |p| {
            (p.y / fila_baja).ceil() as u32
        });
        let hasta = desde.saturating_add(caben);
        hoja.cargar_hasta_fila(hasta);
        if antes.is_some() {
            // Rehecha tras un cambio (una tabla en memoria): una tanda aqui
            // mismo, para que una tabla normal no se vea «cargando» al
            // escribir; lo que quede, en otro hilo como siempre.
            for _ in 0..3 {
                if hoja.cargar_tanda(TANDA_CELDAS / 2) {
                    break;
                }
            }
        }
        let rejilla = Rejilla::nueva_en(&hoja, &libro.estilos.fuente_normal, self.digito, self.ppp);
        let tandas = hoja.tomar_lector().map(|l| leer_aparte(l, self.hwnd));
        let (x, y, sel) = match antes {
            Some(p) => (p.x, p.y, p.sel),
            None => (
                rejilla.x(vh.primera_columna.max(vh.columnas_fijas)),
                rejilla.y(vh.primera_fila.max(vh.filas_fijas)),
                vh.celda_activa.map_or(
                    Seleccion::en(vh.filas_fijas, vh.columnas_fijas),
                    |(f, c)| Seleccion::en(f, c),
                ),
            ),
        };
        tracing::info!(
            ms = ms(t),
            hoja = %hoja.nombre,
            filas = hoja.filas.len(),
            hasta,
            completa = hoja.completa,
            "visor de hojas: primera pantalla leida"
        );
        let mut ab = Abierta {
            hoja,
            rejilla,
            x,
            y,
            zoom,
            sel,
            tandas,
            abierta_en: t,
        };
        let (_, celdas) = vista_de(&ab, self.zonas().hoja);
        acotar(&mut ab, celdas);
        self.hojas[i] = Some(ab);
    }

    /// Cambia a la hoja `i`. Editando, a su tabla (dejando escrito lo que se
    /// tecleaba); una hoja sin tabla (de graficos) no se abre mientras se
    /// edita: se dice.
    fn cambiar_hoja(&mut self, i: usize, motor: &MotorRender, textos: &Catalogo) {
        if i >= self.libro.hojas.len() || i == self.activa {
            return;
        }
        if self.editando {
            let nombres: Vec<&str> = self.libro.hojas.iter().map(|h| h.nombre.as_str()).collect();
            let Some(t) = self
                .ed
                .as_deref()
                .and_then(|ed| ed.tabla_de_vista(&nombres, i))
            else {
                self.avisar(textos.t("hoja-no-editable"));
                return;
            };
            if let Some(e) = self.editor_mut() {
                e.confirmar();
            }
            // Si lo escrito rehizo lo que se ve, las hojas son ya las tablas.
            let i = if self.al_dia(motor) { t } else { i };
            if let Some(e) = self.editor_mut() {
                e.cambiar_hoja(t);
            }
            self.ir_a_hoja(i, motor);
            self.sel_al_editor();
            return;
        }
        self.ir_a_hoja(i, motor);
    }

    fn ir_a_hoja(&mut self, i: usize, motor: &MotorRender) {
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
        if adelante {
            (self.activa + 1..n).find(|&i| visible(i))
        } else {
            (0..self.activa).rev().find(|&i| visible(i))
        }
    }

    /// **Recoge las tandas que llegaron** de todas las hojas. `true` si
    /// cambio algo de la activa.
    fn recibir(&mut self) -> bool {
        let mut cambio_activa = false;
        for (i, slot) in self.hojas.iter_mut().enumerate() {
            let Some(ab) = slot.as_mut() else { continue };
            let Some(rx) = ab.tandas.as_ref() else {
                continue;
            };
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
        let r = Rango {
            fila1: r.fila1.min(uf.max(r.fila0)),
            col1: r.col1.min(uc.max(r.col0)),
            ..r
        };
        let celdas = logica::celdas_de(r);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("celdas", celdas);
        if celdas > logica::MAXIMO_A_COPIAR {
            let t = textos.t_args("hoja-demasiado", &args);
            self.avisar(t);
            return;
        }
        let (libro, hoja) = (&*self.libro, &ab.hoja);
        let t = Instant::now();
        let texto = logica::tsv(r, |f, c| {
            hoja.celda(f, c)
                .and_then(|c| libro.contenido(hoja, c))
                .map(|x| x.mostrado.texto)
                .unwrap_or_default()
        });
        match pixpin_codec::copiar_texto(&texto) {
            Ok(()) => {
                tracing::info!(
                    celdas,
                    bytes = texto.len(),
                    ms = ms(t),
                    "visor de hojas: copiado"
                );
                let t = textos.t_args("hoja-copiado", &args);
                self.avisar(t);
            }
            Err(e) => tracing::warn!(?e, "no se pudo copiar la seleccion"),
        }
    }

    /// Abre el enlace de la celda, si tiene. `true` si tenia.
    fn abrir_enlace(&mut self, (f, c): (u32, u32), textos: &Catalogo, motor: &MotorRender) -> bool {
        let Some(hv) = self.abierta().hoja.hipervinculo_en(f, c) else {
            return false;
        };
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
                    && let Some(i) = self
                        .libro
                        .hojas
                        .iter()
                        .position(|h| h.nombre.eq_ignore_ascii_case(&nombre))
                {
                    self.cambiar_hoja(i, motor, textos);
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
        let marco = logica::Marco {
            ultima: ultima(hoja),
            fijas: (ab.rejilla.filas_fijas, ab.rejilla.columnas_fijas),
            filas_por_pagina,
            lleno: &lleno_en,
        };
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
        let (rx, ry) = foco.map_or((0.0, 0.0), |(x, y)| {
            ((x - celdas.x) as f64, (y - celdas.y) as f64)
        });
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
            self.desde_pestana = if m < 0.0 {
                (self.desde_pestana + 1).min(n.saturating_sub(1))
            } else {
                self.desde_pestana.saturating_sub(1)
            };
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
            ab.y += crate::lector::desplazamiento_de_rueda(delta, ab.rejilla.alto_defecto as f32)
                as f64;
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
        ab.rejilla
            .celda_en_hoja(&ab.hoja, &vista_celdas(&v, celdas), px, py)
    }

    /// Mientras se arrastra fuera de las celdas, la vista corre hacia alli.
    fn correr_al_arrastrar(&mut self) -> bool {
        if !matches!(
            self.gesto,
            Some(Gesto::Elegir { .. } | Gesto::Encabezado(_))
        ) {
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
                ab.y = logica::pos_de_pulgar(
                    b.pista.alto,
                    b.pulgar.alto,
                    raton.1 - agarre - b.pista.y,
                    b.min,
                    b.max,
                );
            }
            (Eje::Horizontal, _, Some(b)) => {
                ab.x = logica::pos_de_pulgar(
                    b.pista.ancho,
                    b.pulgar.ancho,
                    raton.0 - agarre - b.pista.x,
                    b.min,
                    b.max,
                );
            }
            _ => {}
        }
    }

    /// **Pulsa el boton central**: empieza a mover la hoja con la mano.
    fn empezar_mano(&mut self) {
        let ab = self.abierta();
        self.gesto = Some(Gesto::Mano {
            desde: self.raton,
            x: ab.x,
            y: ab.y,
        });
    }

    /// Moviendo con el central: la hoja sigue al raton (lo que se movio en
    /// pantalla, al zoom que se ve), sin pasarse de sus limites.
    fn llevar_mano(&mut self) {
        let Some(Gesto::Mano { desde, x, y }) = self.gesto else {
            return;
        };
        let (dx, dy) = (self.raton.0 - desde.0, self.raton.1 - desde.1);
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        let z = ab.zoom.max(0.01);
        ab.x = x - dx as f64 / z;
        ab.y = y - dy as f64 / z;
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
    }

    /// **Un clic** en la ventana. `true` si empieza un arrastre (hay que
    /// capturar el raton).
    fn pulsar(&mut self, motor: &MotorRender, textos: &Catalogo) -> bool {
        // El riel (marcar, el lapiz, el marco y las marcas) va encima de todo.
        if self.pulsar_riel(motor, textos) {
            return false;
        }
        // Con los marcos, sus fichas de abajo (1, 2, 3…) llevan a cada uno.
        if let Some(k) = self.ficha_bajo_el_raton() {
            self.ir_al_marco(k, motor, textos);
            return false;
        }
        let p = self.raton;
        let z = self.zonas();
        // Las pestanas.
        if dentro(&z.pestanas, p) {
            if let Some(t) = self.pestanas.iter().find(|t| dentro(&t.caja, p)) {
                let i = t.hoja;
                self.cambiar_hoja(i, motor, textos);
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
            let agarre = if en >= desde && en < desde + largo {
                en - desde
            } else {
                largo / 2.0
            };
            self.gesto = Some(Gesto::Barra { eje, agarre });
            self.encima_de = Some(eje);
            self.llevar_barra(eje, agarre);
            return true;
        }
        // Con los marcos, arrastrar en las celdas pone uno (no elige).
        if self.marcos.activo {
            return self.pulsar_marco();
        }
        // Anotando, las celdas son de la tinta (lo que no quiso, nada).
        if self.anotando {
            return false;
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
                    ab.sel = Seleccion {
                        ancla: (0, 0),
                        activa: (uf, uc),
                    };
                    return false;
                }
                (true, false, Some((f, _))) => (
                    Eje::Vertical,
                    Seleccion {
                        ancla: (f, 0),
                        activa: (f, uc),
                    },
                ),
                (false, true, Some((_, c))) => (
                    Eje::Horizontal,
                    Seleccion {
                        ancla: (0, c),
                        activa: (uf, c),
                    },
                ),
                _ => return false,
            };
            ab.sel = if mods.shift {
                Seleccion {
                    ancla: ab.sel.ancla,
                    activa: sel.activa,
                }
            } else {
                sel
            };
            self.gesto = Some(Gesto::Encabezado(eje));
            return true;
        }
        let Some(celda) = self.celda_bajo(p, false) else {
            return false;
        };
        if !self.editando && self.clic_en_flecha(p, celda) {
            self.menu_de_lista(celda);
            return false;
        }
        let ab = self.abierta_mut();
        ab.sel = if mods.shift {
            Seleccion {
                ancla: ab.sel.ancla,
                activa: celda,
            }
        } else {
            Seleccion::en(celda.0, celda.1)
        };
        if self.editando && !mods.shift {
            // Dos clics en la misma celda: escribir en ella con lo que tiene.
            let doble = self
                .ultimo_clic
                .is_some_and(|(c, t)| c == celda && t.elapsed() < DOBLE_CLIC);
            if doble {
                self.ultimo_clic = None;
                self.sel_al_editor();
                if let Some(e) = self.editor_mut() {
                    e.empezar(true);
                }
                return false;
            }
            self.ultimo_clic = Some((celda, Instant::now()));
        }
        self.gesto = Some(Gesto::Elegir {
            pulsada: celda,
            cuando: Instant::now(),
            ctrl: mods.ctrl,
        });
        true
    }

    /// La celda bajo el raton si lleva una lista desplegable (en la zona de
    /// celdas, no en los encabezados ni las barras).
    fn celda_con_lista_bajo_el_raton(&self) -> Option<(u32, u32)> {
        let p = self.raton;
        let (_, celdas) = self.vista();
        if !dentro(&celdas, p) {
            return None;
        }
        let c = self.celda_bajo(p, false)?;
        self.abierta().hoja.lista_en(c.0, c.1).map(|_| c)
    }

    /// La celda bajo el raton si tiene un comentario o una nota.
    fn celda_con_comentario_bajo_el_raton(&self) -> Option<(u32, u32)> {
        let p = self.raton;
        let (_, celdas) = self.vista();
        if !dentro(&celdas, p) {
            return None;
        }
        let c = self.celda_bajo(p, false)?;
        // Una celda combinada lleva el comentario en su esquina de arriba a la izquierda.
        let ab = self.abierta();
        let c = ab
            .hoja
            .combinada_en(c.0, c.1)
            .map_or(c, |r| (r.fila0, r.col0));
        ab.hoja.comentario_en(c.0, c.1).map(|_| c)
    }

    /// El boton de la flecha de la lista de `celda` en pantalla, si la tiene y se ve.
    fn caja_de_flecha(&self, celda: (u32, u32)) -> Option<[f32; 4]> {
        let ab = self.abierta();
        ab.hoja.lista_en(celda.0, celda.1)?;
        let (v, celdas) = self.vista();
        let caja = hoja_office::caja_de_celda(
            &ab.rejilla,
            &vista_celdas(&v, celdas),
            celdas,
            celda.0,
            celda.1,
        )?;
        hoja_office::caja_flecha(caja, (ab.zoom * ab.rejilla.ppp / 96.0) as f32)
    }

    fn clic_en_flecha(&self, (x, y): (f32, f32), celda: (u32, u32)) -> bool {
        self.caja_de_flecha(celda)
            .is_some_and(|[x0, y0, x1, y1]| x >= x0 && x < x1 && y >= y0 && y < y1)
    }

    /// **Los valores permitidos** de la lista de `celda`, en un menu de solo
    /// lectura (elegir uno no cambia nada: el visor no edita). Marca el que
    /// tiene la celda ahora.
    fn menu_de_lista(&mut self, celda: (u32, u32)) {
        let ab = self.abierta();
        let Some(regla) = ab.hoja.lista_en(celda.0, celda.1).cloned() else {
            return;
        };
        let valores = pixpin_hojas::validacion::valores_lista(&*self.libro, &ab.hoja, &regla);
        if valores.is_empty() {
            return;
        }
        let actual = ab
            .hoja
            .celda(celda.0, celda.1)
            .and_then(|c| self.libro.contenido(&ab.hoja, c))
            .map(|c| c.mostrado.texto)
            .unwrap_or_default();
        let entradas: Vec<(u32, String)> = valores
            .iter()
            .enumerate()
            .map(|(i, v)| {
                (
                    i as u32 + 1,
                    if *v == actual {
                        format!("\u{2713} {v}")
                    } else {
                        v.clone()
                    },
                )
            })
            .collect();
        let _ = pixpin_shell::menu_llano(
            windows::Win32::Foundation::HWND(self.hwnd as *mut _),
            &entradas,
        );
    }

    /// Al soltar: un clic corto en una celda con enlace lo abre.
    fn soltar(&mut self, textos: &Catalogo, motor: &MotorRender) {
        let gesto = self.gesto.take();
        if let Some(Gesto::Marco) = gesto {
            self.soltar_marco();
            return;
        }
        if let Some(Gesto::Elegir {
            pulsada, cuando, ..
        }) = gesto
            && !self.editando
        {
            let sel = self.abierta().sel;
            if sel.ancla == pulsada && sel.activa == pulsada && cuando.elapsed() < CLIC_DE_ENLACE {
                self.abrir_enlace(pulsada, textos, motor);
            }
        }
    }

    /// El cursor que toca: la mano sobre un enlace.
    fn cursor(&self) -> FormaCursorWin {
        if matches!(self.gesto, Some(Gesto::Mano { .. })) {
            return FormaCursorWin::Mover;
        }
        // Sobre una ficha de los marcos, la mano (IDC_HAND): lleva a su marco.
        if self.gesto.is_none() && self.ficha_bajo_el_raton().is_some() {
            return FormaCursorWin::Giro;
        }
        if self.marcos.activo
            && (matches!(self.gesto, Some(Gesto::Marco)) || dentro(&self.vista().1, self.raton))
        {
            return FormaCursorWin::Cruz;
        }
        if self.gesto.is_none()
            && !self.editando
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
// Editar (una hoja del chat): lo que el visor hace con `hoja_pin`

/// La celda del visor (fila, columna) como la del editor.
fn a_ref((fila, columna): (u32, u32)) -> pixpin_proyecto::tabla::Ref {
    pixpin_proyecto::tabla::Ref { columna, fila }
}

fn de_ref(r: pixpin_proyecto::tabla::Ref) -> (u32, u32) {
    (r.fila, r.columna)
}

impl<'l> Estado<'l> {
    fn editor(&self) -> Option<&crate::hoja_pin::editor::Editor> {
        self.ed.as_deref().and_then(|ed| ed.editor())
    }

    fn editor_mut(&mut self) -> Option<&mut crate::hoja_pin::editor::Editor> {
        self.ed.as_deref_mut().and_then(|ed| ed.editor_mut())
    }

    /// Escribiendo en una celda.
    fn escribiendo(&self) -> bool {
        self.editando && self.editor().is_some_and(|e| e.edicion.is_some())
    }

    /// Lo elegido en el visor pasa al editor (lo que se toca es eso).
    fn sel_al_editor(&mut self) {
        if !self.editando || self.hojas.get(self.activa).is_none_or(|h| h.is_none()) {
            return;
        }
        let s = self.abierta().sel;
        if let Some(e) = self.editor_mut() {
            e.poner_seleccion(crate::hoja_pin::editor::Seleccion {
                ancla: a_ref(s.ancla),
                activa: a_ref(s.activa),
            });
        }
    }

    /// Y al reves (deshacer y pegar mueven lo elegido), dejandolo a la vista.
    fn sel_del_editor(&mut self) {
        let Some(s) = self.editor().map(|e| e.hoja().sel) else {
            return;
        };
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        ab.sel = Seleccion {
            ancla: de_ref(s.ancla),
            activa: de_ref(s.activa),
        };
        let (_, celdas) = vista_de(ab, caja);
        asegurar_visible(ab, celdas, de_ref(s.activa));
    }

    /// **Lo que se ve, al dia con lo editado**: si el editor cambio algo, se
    /// rehace el libro con sus tablas y se vuelve a abrir la hoja en el
    /// MISMO sitio (zoom, desplazamiento y lo elegido): no salta nada. Las
    /// otras hojas abiertas se recuerdan donde estaban y se abren al
    /// volver a ellas. `true` si se rehizo.
    fn al_dia(&mut self, motor: &MotorRender) -> bool {
        let Some(ed) = self.ed.as_deref_mut() else {
            return false;
        };
        let era_original = ed.vista_original();
        let Some(nuevo) = ed.libro_al_dia() else {
            return false;
        };
        let Some(e) = ed.editor() else { return false };
        let tabla = e.activa;
        let sel = e.hoja().sel;
        let t = Instant::now();
        // Viendo el original las hojas no son las tablas: solo se conserva
        // la que se edita.
        let mut guardadas: std::collections::HashMap<usize, Posicion> = if era_original {
            Default::default()
        } else {
            self.hojas
                .iter()
                .enumerate()
                .filter_map(|(i, h)| Some((i, h.as_ref()?.posicion())))
                .collect()
        };
        if let Some(ab) = self.hojas.get(self.activa).and_then(|h| h.as_ref()) {
            let mut p = ab.posicion();
            p.sel = Seleccion {
                ancla: de_ref(sel.ancla),
                activa: de_ref(sel.activa),
            };
            guardadas.insert(tabla, p);
        }
        *self.libro = nuevo;
        self.generacion += 1;
        let n = self.libro.hojas.len();
        self.hojas = (0..n).map(|_| None).collect();
        self.guardadas = guardadas;
        self.activa = tabla.min(n.saturating_sub(1));
        self.desde_pestana = self.desde_pestana.min(self.activa);
        self.gesto = None;
        let activa = self.activa;
        self.abrir_hoja(activa, motor);
        tracing::info!(ms = ms(t), hojas = n, "hoja del chat: lo editado ya se ve");
        true
    }

    /// **Pasa a editar** (con «Editar», F2 o la `E`): las tablas se leen la
    /// primera vez; la hoja que se ve es la que se edita, con lo elegido.
    /// `false` si no se puede (sin tablas, o una hoja de graficos).
    fn empezar_a_editar(&mut self, textos: &Catalogo) -> bool {
        if self.editando {
            return true;
        }
        let Some(ed) = self.ed.as_deref_mut() else {
            return false;
        };
        if let Err(e) = ed.montar_editor() {
            tracing::warn!(%e, "hoja del chat: no se puede editar");
            self.avisar(textos.t("hoja-no-editable"));
            return false;
        }
        let nombres: Vec<&str> = self.libro.hojas.iter().map(|h| h.nombre.as_str()).collect();
        let Some(t) = ed.tabla_de_vista(&nombres, self.activa) else {
            self.avisar(textos.t("hoja-no-editable"));
            return false;
        };
        let aviso = ed.aviso_al_editar();
        if let Some(e) = ed.editor_mut() {
            e.cancelar();
            e.cambiar_hoja(t);
        }
        // Escribir en las celdas, anotar con el lapiz y poner marcos no van a
        // la vez.
        self.dejar_de_anotar();
        if self.marcos.activo {
            self.marcos.alternar();
        }
        self.editando = true;
        self.ultimo_clic = None;
        self.sel_al_editor();
        if let Some(clave) = aviso {
            self.aviso = Some((textos.t(clave), Instant::now() + AVISO_LARGO));
        }
        tracing::info!(tabla = t, "hoja del chat: modo editar");
        true
    }

    /// **Vuelve a ver**: deja escrito lo que se tecleaba y lo guarda en el
    /// chat (nunca en el archivo). Lo que se ve ya esta al dia.
    fn dejar_de_editar(&mut self, motor: &MotorRender) {
        if !self.editando {
            return;
        }
        if let Some(e) = self.editor_mut() {
            e.confirmar();
        }
        self.al_dia(motor);
        if let Some(ed) = self.ed.as_deref_mut()
            && let Err(e) = ed.guardar()
        {
            tracing::error!(%e, "hoja del chat: no se pudo guardar; los cambios siguen en el pin");
        }
        self.editando = false;
        self.ultimo_clic = None;
        tracing::info!("hoja del chat: modo ver");
    }

    /// Una letra tecleada editando: entra en la celda activa (de cero si no
    /// se escribia, como en cualquier hoja).
    fn teclear(&mut self, c: char) {
        if self.sin_letras_hasta.is_some_and(|t| Instant::now() < t) {
            return;
        }
        if !self.escribiendo() {
            self.sel_al_editor();
        }
        if let Some(e) = self.editor_mut() {
            e.teclear(c);
        }
    }

    /// **Lo que se hace editando** (teclas y botones). `true` si se vuelve
    /// a ver.
    fn hacer(
        &mut self,
        a: crate::hoja_pin::AccionEd,
        textos: &Catalogo,
        motor: &MotorRender,
    ) -> bool {
        use crate::hoja_pin::AccionEd as A;
        if !self.escribiendo() {
            self.sel_al_editor();
        }
        let mueve_lo_elegido = matches!(a, A::Deshacer | A::Rehacer | A::Pegar);
        let mut aviso = None;
        {
            let Some(e) = self.editor_mut() else {
                return false;
            };
            let grande = e.hoja().sel.celdas() > crate::hoja_pin::editor::MAXIMO_RANGO;
            match a {
                A::Ver => return true,
                A::Confirmar => {
                    e.confirmar();
                }
                A::Empezar => e.empezar(true),
                A::Cancelar => e.cancelar(),
                A::Retroceso => e.retroceso(),
                A::Borrar => {
                    if !e.borrar_seleccion() && grande {
                        aviso = Some(textos.t("hoja-rango-grande"));
                    }
                }
                A::Deshacer => {
                    e.deshacer();
                }
                A::Rehacer => {
                    e.rehacer();
                }
                A::Copiar => aviso = crate::hoja_pin::copiar(e, textos),
                A::Cortar => {
                    aviso = crate::hoja_pin::copiar(e, textos);
                    if !grande {
                        e.borrar_seleccion();
                    }
                }
                A::Pegar => crate::hoja_pin::pegar(e),
                A::Formato(f) => {
                    if !e.formato(f) && grande {
                        aviso = Some(textos.t("hoja-rango-grande"));
                    }
                }
                A::Nada => {}
            }
        }
        if let Some(t) = aviso {
            self.avisar(t);
        }
        self.al_dia(motor);
        if mueve_lo_elegido {
            self.sel_del_editor();
        }
        false
    }

    /// Un boton de la barra al editar (los menus de colores y de formato de
    /// numero salen junto al raton). `true` si se vuelve a ver.
    fn boton_de_editar(&mut self, id: u8, textos: &Catalogo, motor: &MotorRender) -> bool {
        use crate::hoja_pin::{AccionEd as A, PideBoton as P};
        let Some(pide) = crate::hoja_pin::pide_boton(id) else {
            return false;
        };
        let accion = match pide {
            P::Ver => return true,
            P::Deshacer => A::Deshacer,
            P::Rehacer => A::Rehacer,
            P::Formato(f) => A::Formato(f),
            P::Menu(m) => {
                self.sel_al_editor();
                let Some(e) = self.editor() else { return false };
                let entradas = crate::hoja_pin::entradas_de_menu(m, e, textos);
                let rotulos: Vec<(u32, String)> =
                    entradas.iter().map(|(i, t, _)| (*i, t.clone())).collect();
                let elegido = pixpin_shell::menu_llano(
                    windows::Win32::Foundation::HWND(self.hwnd as *mut _),
                    &rotulos,
                );
                let Some(f) = elegido
                    .and_then(|id| entradas.into_iter().find(|(i, ..)| *i == id))
                    .map(|(_, _, f)| f)
                else {
                    return false;
                };
                A::Formato(f)
            }
        };
        self.hacer(accion, textos, motor)
    }
}

// ---------------------------------------------------------------------------
// La ventana

/// La escala del monitor donde cae el punto (o la del principal).
fn escala_en(p: Punto) -> u32 {
    let Ok(d) = pixpin_capture::enumerar_monitores() else {
        return 100;
    };
    d.monitores()
        .iter()
        .find(|m| {
            p.x >= m.area.x
                && p.y >= m.area.y
                && p.x < m.area.x + m.area.ancho as i32
                && p.y < m.area.y + m.area.alto as i32
        })
        .or_else(|| d.principal())
        .map_or(100, |m| m.escala_por_cien)
}

/// **La ventana del pin de la hoja**: la abre y la tiene hasta que se
/// cierra, viendo o (si la hoja es del chat) editando, sin cambiar de
/// ventana ni de pintor.
#[allow(clippy::too_many_arguments)] // lo que se abre y de donde viene
fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    libro: &mut Libro,
    ruta: Option<&Path>,
    nombre: &str,
    t0: Instant,
    editable: Option<&mut crate::hoja_pin::Editable>,
) -> Result<()> {
    // Cuenta como abierta para los grupos de ventanas (H9) mientras viva.
    let _grupo = ruta.map(|r| {
        crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lector {
            ruta: r.to_path_buf(),
        })
    });
    if libro.hojas.is_empty() {
        anyhow::bail!("el libro no tiene hojas");
    }
    let (area_pin, _) = crate::lector_pin::area_inicial();
    let escala = escala_en(Punto {
        x: area_pin.x + area_pin.ancho as i32 / 2,
        y: area_pin.y + area_pin.alto as i32 / 2,
    });
    let e = escala as f32 / 100.0;
    let ext = ruta
        .map(|r| pixpin_docs::extension(&pixpin_docs::nombre(r)))
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "xlsx".into());
    let pin = PinLector::nuevo(area_pin, e).con_extension(&ext);
    let area = pin.area;
    let ventana = VentanaOverlay::nueva_normal(area, nombre)
        .context("no se pudo abrir la ventana del visor de hojas")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        area.ancho,
        area.alto,
    )
    .context("sin superficie para el visor de hojas")?;
    ventana.mostrar();
    ventana.enfocar();
    // Como un pin pero sin fijar (lo fija su pin): al frente, una ventana
    // mas entre las demas, y su barra fuera.
    pin.al_nacer(&ventana);
    let mut marco = Marco {
        ventana,
        superficie,
        pin,
        area,
        e,
        nombre: nombre.to_string(),
    };
    marco
        .pin
        .crear_barra(&motor, &recursos.d3d(), &marco.ventana);
    let activa = libro.hoja_activa.min(libro.hojas.len().saturating_sub(1));
    ver_en(marco, &motor, textos, libro, activa, ruta, editable, t0);
    Ok(())
}

/// **El bucle de la ventana**: ver y editar son el MISMO visor (la misma
/// hoja abierta, el mismo zoom y la misma posicion); editar solo cambia los
/// botones de la barra, lo que hacen algunas teclas y la barra de formulas.
#[allow(clippy::too_many_arguments)] // la ventana y lo que se ve en ella
fn ver_en(
    marco: Marco,
    motor: &MotorRender,
    textos: &Catalogo,
    libro: &mut Libro,
    activa: usize,
    ruta: Option<&Path>,
    editable: Option<&mut crate::hoja_pin::Editable>,
    t0: Instant,
) {
    let Marco {
        mut ventana,
        superficie,
        mut pin,
        mut area,
        e,
        nombre,
    } = marco;
    let al_abrir_editar = editable.as_ref().is_some_and(|e| e.al_abrir_editar);
    let n = libro.hojas.len();
    let (recuerdo, marcas) = riel::Recuerdo::de(ruta);
    let mut est = Estado {
        libro,
        ed: editable,
        editando: false,
        ultimo_clic: None,
        sin_letras_hasta: None,
        generacion: 0,
        guardadas: Default::default(),
        hojas: (0..n).map(|_| None).collect(),
        activa: activa.min(n.saturating_sub(1)),
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
        bajo_raton: None,
        comentario_bajo: None,
        aviso: None,
        hwnd: ventana.handle().0 as isize,
        anotado: Default::default(),
        nombre: nombre.clone(),
        area,
        anotando: false,
        poniendo_marca: false,
        encima: pixpin_ui::riel_marcas::DestinoRiel::Fuera,
        marcas,
        recuerdo,
        tinta_cambio: false,
        trazo: false,
        marcos: crate::marcos_doc::Marcos::de(ruta),
        vistas: crate::marcos_doc::Vistas::de_documento(ruta),
        sin_interfaz: false,
    };
    let activa = est.activa;
    est.abrir_hoja(activa, motor);
    est.desde_pestana = activa.saturating_sub(3);
    // Una tabla recien creada se abre ya editando.
    let mut quiere_editar = al_abrir_editar;

    let editable = est.ed.is_some();
    let mut hay_que_pintar = true;
    let mut primera = true;
    let mut vivo = true;
    // Ctrl+P: los marcos (o lo que se ve), en grupo al dialogo de imprimir.
    let mut imprimir_marcos = false;
    // Para que modo estan puestas las pistas de la barra (ver o editar).
    let mut pistas_de: Option<bool> = None;
    // Pruebas: imprimir los marcos solo, a los 2 s de abrir (con
    // `PIXPIN_IMPRIMIR_A_PDF`, a un fichero), sin Ctrl+P de verdad.
    let mut imprimir_al_abrir = std::env::var_os("PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR")
        .map(|_| Instant::now() + Duration::from_secs(2));
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        // Las ordenes de las pruebas del pin de hoja (`hoja_pin::prueba`), y
        // lo que necesitan saber de lo que se ve.
        #[cfg(test)]
        crate::hoja_pin::prueba::apuntar_vista((
            est.marcos
                .fichas(est.zonas().pestanas.y, est.e)
                .iter()
                .map(|r| ((r.x + r.ancho / 2.0) as i32, (r.y + r.alto / 2.0) as i32))
                .collect(),
            est.hojas
                .get(est.activa)
                .and_then(Option::as_ref)
                .map_or(1.0, |ab| ab.zoom),
            [est.poniendo_marca, est.anotando, est.marcos.activo],
        ));
        #[cfg(test)]
        match crate::hoja_pin::prueba::latido(ventana.handle().0 as isize, area, est.editando) {
            Some(crate::hoja_pin::prueba::EDITAR) => quiere_editar = true,
            Some(crate::hoja_pin::prueba::VER) => {
                est.dejar_de_editar(motor);
                hay_que_pintar = true;
            }
            Some(crate::hoja_pin::prueba::ESCRIBIR) => {
                if est.editando {
                    if let Some(ed) = est.ed.as_deref_mut() {
                        crate::hoja_pin::escribir_muestra(ed);
                    }
                    est.al_dia(motor);
                    est.sel_del_editor();
                    hay_que_pintar = true;
                }
            }
            Some(crate::hoja_pin::prueba::CERRAR) => vivo = false,
            Some(crate::hoja_pin::prueba::MARCOS) => {
                est.alternar_marcos(motor);
                hay_que_pintar = true;
            }
            Some(crate::hoja_pin::prueba::IMPRIMIR) => imprimir_marcos = true,
            _ => {}
        }
        if std::mem::take(&mut quiere_editar) && editable && !est.editando {
            // Leer un libro grande como tablas tarda: que se vea que se esta
            // en ello (la primera vez; despues ya estan).
            if est.editor().is_none() {
                est.aviso = Some((textos.t("hoja-preparando"), Instant::now() + AVISO));
                pintar_ya(&superficie, motor, &mut est, textos);
                est.aviso = None;
            }
            est.empezar_a_editar(textos);
            hay_que_pintar = true;
        }
        #[allow(unused_mut)]
        let mut eventos = pixpin_shell::overlay::tomar_eventos_pendientes();
        // Pruebas: las teclas que manda la prueba, como si se pulsaran aqui
        // (sin el teclado de verdad).
        #[cfg(test)]
        eventos.extend(
            crate::hoja_pin::prueba::tomar_teclas()
                .into_iter()
                .map(|(vk, ctrl)| {
                    (
                        ventana.handle(),
                        EventoOverlay::Tecla {
                            vk,
                            shift: false,
                            ctrl,
                            alt: false,
                        },
                    )
                }),
        );
        for (hwnd, evento) in eventos {
            if pin.es_de_la_barra(hwnd) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(AccionPin::Propio(id)) if est.editando => {
                        if est.boton_de_editar(id, textos, motor) {
                            est.dejar_de_editar(motor);
                        }
                    }
                    // «Abrir con…»: el cuadro de Windows con las apps que
                    // abren el formato; si no sale, el motivo en el aviso.
                    Some(AccionPin::Propio(BOTON_ABRIR_CON)) => {
                        if let Some(ruta) = ruta
                            && let Err(aviso) =
                                crate::abrir_con::abrir_con(ventana.handle(), ruta, textos)
                        {
                            est.aviso = Some((aviso, Instant::now() + AVISO));
                        }
                    }
                    Some(AccionPin::Propio(BOTON_EDITAR)) => quiere_editar = true,
                    Some(AccionPin::PantallaCompleta) => {
                        pin.alternar_pantalla_completa(&mut ventana)
                    }
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
            // Anotando con el lapiz del riel, la tinta primero (`riel`).
            if est.anotando && est.tinta_primero(&evento, &mut ventana, area) {
                hay_que_pintar = true;
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Despierta => {}
                // El derecho en un punto del riel quita su marca; en lo demas,
                // con marcos puestos, su menu (imprimirlos o borrarlos).
                EventoOverlay::BotonDerechoPulsado(p) => {
                    est.raton = local(p);
                    if est.pulsar_derecho_riel() {
                        hay_que_pintar = true;
                    } else if let Some(pedido) = est.menu_de_marcos(textos) {
                        imprimir_marcos |= pedido == crate::marcos_doc::Pedido::Imprimir;
                        hay_que_pintar = true;
                    }
                }
                // Con la tira de las marcas abierta, las cifras eligen el
                // emoticono (1-9 y 0, como en el PDF).
                EventoOverlay::Caracter(c) if est.poniendo_marca && !est.editando => {
                    if let Some(i) = crate::lector::emoji_de_cifra(c) {
                        est.poner_marca(i);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::RatonMovido(p) => {
                    est.raton = local(p);
                    hay_que_pintar |= est.mirar_riel();
                    match est.gesto {
                        Some(Gesto::Barra { eje, agarre }) => {
                            est.llevar_barra(eje, agarre);
                            hay_que_pintar = true;
                        }
                        Some(Gesto::Mano { .. }) => {
                            est.llevar_mano();
                            hay_que_pintar = true;
                        }
                        Some(Gesto::Marco) => hay_que_pintar |= est.mover_marco(),
                        Some(_) => {
                            let antes = est.abierta().sel;
                            est.arrastrar_seleccion();
                            hay_que_pintar |= est.abierta().sel != antes;
                        }
                        None => {
                            let bajo = est.celda_con_lista_bajo_el_raton();
                            if bajo != est.bajo_raton {
                                est.bajo_raton = bajo;
                                hay_que_pintar = true;
                            }
                            let con_nota = est.celda_con_comentario_bajo_el_raton();
                            if con_nota != est.comentario_bajo {
                                est.comentario_bajo = con_nota;
                                hay_que_pintar = true;
                            }
                            let encima = est.barra_bajo_el_raton();
                            if encima != est.encima_de {
                                est.encima_de = encima;
                                hay_que_pintar = true;
                            }
                        }
                    }
                    ventana.poner_cursor(est.cursor());
                }
                EventoOverlay::BotonPulsado(_) if matches!(est.gesto, Some(Gesto::Mano { .. })) => {
                }
                EventoOverlay::BotonPulsado(p) => {
                    est.raton = local(p);
                    // Escribiendo, un clic fuera de la celda deja escrito lo
                    // tecleado (uno en ella misma sigue escribiendo).
                    if est.escribiendo()
                        && est.celda_bajo(est.raton, false) != Some(est.abierta().sel.activa)
                    {
                        est.hacer(crate::hoja_pin::AccionEd::Confirmar, textos, motor);
                    }
                    if !(est.escribiendo()
                        && est.celda_bajo(est.raton, false) == Some(est.abierta().sel.activa))
                    {
                        let ctrl = pixpin_shell::entrada::modificadores_pulsados().ctrl;
                        if est.pulsar(motor, textos) {
                            ventana.capturar_raton();
                        }
                        // Ctrl+clic en un enlace lo abre ya (viendo).
                        if ctrl
                            && !est.editando
                            && let Some(Gesto::Elegir { pulsada, .. }) = est.gesto
                            && est.abrir_enlace(pulsada, textos, motor)
                        {
                            est.gesto = None;
                            ventana.soltar_raton();
                        }
                    }
                    hay_que_pintar = true;
                }
                // El boton central (la rueda) pulsado mueve la hoja, como el
                // lienzo: cursor de mover y sin elegir celdas.
                EventoOverlay::BotonCentralPulsado(p) if est.gesto.is_none() => {
                    est.raton = local(p);
                    est.empezar_mano();
                    ventana.capturar_raton();
                    ventana.poner_cursor(est.cursor());
                }
                EventoOverlay::BotonCentralSoltado(p) => {
                    if matches!(est.gesto, Some(Gesto::Mano { .. })) {
                        est.raton = local(p);
                        est.llevar_mano();
                        est.gesto = None;
                        ventana.soltar_raton();
                        ventana.poner_cursor(est.cursor());
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::BotonSoltado(_) if matches!(est.gesto, Some(Gesto::Mano { .. })) => {
                }
                EventoOverlay::BotonSoltado(p) => {
                    est.raton = local(p);
                    est.soltar(textos, motor);
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
                EventoOverlay::Caracter(c) if est.editando && c >= ' ' => {
                    est.teclear(c);
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, ctrl, alt, .. }
                    if editable && !est.editando && pide_editar(vk, ctrl, alt) =>
                {
                    quiere_editar = true;
                    // La letra de la `E` llega despues: no se escribe.
                    est.sin_letras_hasta = Some(Instant::now() + SIN_LETRAS);
                }
                // Esc primero cierra la tira de las marcas, deja de anotar o
                // sale de los marcos.
                EventoOverlay::Tecla {
                    vk: logica::VK_ESCAPE,
                    ..
                } if est.poniendo_marca || est.anotando || est.marcos.activo => {
                    if est.poniendo_marca {
                        est.poniendo_marca = false;
                    } else if est.anotando {
                        est.dejar_de_anotar();
                    } else {
                        est.alternar_marcos(motor);
                    }
                    hay_que_pintar = true;
                }
                // Los marcos, como en el plano: I los pone o los quita, Supr
                // (con ellos puestos) quita el ultimo y Ctrl+P imprime todos.
                EventoOverlay::Tecla {
                    vk,
                    ctrl,
                    alt: false,
                    shift: false,
                } if !est.escribiendo() && est.tecla_de_marcos(vk, ctrl, motor).is_some() => {
                    if ctrl && vk == crate::marcos_doc::VK_P {
                        imprimir_marcos = true;
                    }
                    hay_que_pintar = true;
                }
                // Las teclas del riel, las mismas en todos los visores: M (o
                // Ctrl+M) el marcador, A el lapiz. Viendo: editando, las
                // letras son de las celdas.
                EventoOverlay::Tecla {
                    vk: VK_M,
                    alt: false,
                    shift: false,
                    ..
                } if !est.editando => {
                    est.poniendo_marca = !est.poniendo_marca;
                    if est.poniendo_marca && est.marcos.activo {
                        est.alternar_marcos(motor);
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla {
                    vk: VK_A,
                    ctrl: false,
                    alt: false,
                    shift: false,
                } if !est.editando && !est.anotando => {
                    est.alternar_anotar(motor);
                    hay_que_pintar = true;
                }
                // Ctrl+1…9 llevan a esa marca del riel, como en el PDF.
                EventoOverlay::Tecla {
                    vk,
                    ctrl: true,
                    alt: false,
                    shift: false,
                } if !est.editando
                    && (0x31..=0x39).contains(&vk)
                    && ((vk - 0x31) as usize) < est.marcas.len() =>
                {
                    est.ir_a_la_marca((vk - 0x31) as usize, motor, textos);
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla {
                    vk,
                    shift,
                    ctrl,
                    alt,
                } => {
                    use crate::hoja_pin::AccionEd;
                    let al_visor = if est.editando && !alt {
                        match crate::hoja_pin::accion_de_tecla(vk, shift, ctrl, est.escribiendo()) {
                            Some(AccionEd::Ver) => {
                                est.dejar_de_editar(motor);
                                false
                            }
                            // Lo escrito queda y la tecla mueve, como en el visor.
                            Some(AccionEd::Confirmar) => {
                                est.hacer(AccionEd::Confirmar, textos, motor);
                                true
                            }
                            Some(a) => {
                                est.hacer(a, textos, motor);
                                false
                            }
                            None => true,
                        }
                    } else {
                        true
                    };
                    // Ctrl+E: el «Exportar» comun (PDF, HTML, .pixpin, imagen, CSV, original).
                    // Ctrl+Z / Ctrl+Y viendo: lo ultimo que anoto el asistente.
                    if al_visor && !est.editando && ctrl && !alt && (vk == VK_Z || vk == VK_Y) {
                        let hecho = if vk == VK_Z && !shift {
                            est.anotado.deshacer()
                        } else {
                            est.anotado.rehacer()
                        };
                        hay_que_pintar |= hecho;
                        if hecho {
                            continue;
                        }
                    }
                    if al_visor && ctrl && !alt && vk == VK_E {
                        exportar_libro(ruta);
                    } else if al_visor {
                        vivo &= tecla_del_visor(
                            &mut est,
                            &mut pin,
                            &mut ventana,
                            (vk, shift, ctrl, alt),
                            textos,
                            motor,
                        );
                    }
                    hay_que_pintar = true;
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // Lo que anoto el asistente en esta hoja, a la vista.
        if est.anotado.mirar(ruta, est.activa) {
            hay_que_pintar = true;
        }
        if est.forma_rapida() {
            hay_que_pintar = true;
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
            est.area = area;
            est.ancho = area.ancho as f32;
            est.alto = area.alto as f32;
            let caja = est.zonas().hoja;
            let ab = est.abierta_mut();
            let (_, celdas) = vista_de(ab, caja);
            acotar(ab, celdas);
            hay_que_pintar = true;
        }
        // El nombre y la tecla de los botones propios de la barra, para su
        // globo: cambian al pasar de ver a editar.
        if pistas_de != Some(est.editando) {
            pistas_de = Some(est.editando);
            pin.poner_pistas(if est.editando {
                crate::hoja_pin::pistas_de_editar(textos)
            } else {
                pistas_de_ver(textos)
            });
        }
        let botones = match est.editor() {
            Some(ed) if est.editando => crate::hoja_pin::botones_de_editar(ed),
            _ => botones_de_ver(editable, ruta.is_some()),
        };
        // La luna/sol de la barra cambia el modo oscuro: se repinta (viendo
        // y editando, que es el mismo pintor).
        if pin.actualizar(motor, &nombre, &botones) {
            hay_que_pintar = true;
        }
        if est
            .aviso
            .as_ref()
            .is_some_and(|(_, hasta)| Instant::now() >= *hasta)
        {
            est.aviso = None;
            hay_que_pintar = true;
        }

        if imprimir_al_abrir.is_some_and(|t| Instant::now() >= t) {
            imprimir_al_abrir = None;
            imprimir_marcos = true;
        }
        if std::mem::take(&mut imprimir_marcos) {
            if est.gesto.is_some() {
                est.gesto = None;
                ventana.soltar_raton();
            }
            marcos::imprimir(&superficie, motor, &mut est, textos);
            hay_que_pintar = true;
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            let t = Instant::now();
            pintar_ya(&superficie, motor, &mut est, textos);
            if primera {
                primera = false;
                tracing::info!(
                    ms = ms(t0),
                    pintar_ms = ms(t),
                    "visor de hojas: primera pantalla pintada"
                );
            }
        }
        let tope = if est.gesto.is_some() || est.trazando() {
            16
        } else if est.cargando()
            || pin.quiere_reloj()
            || est.aviso.is_some()
            || imprimir_al_abrir.is_some()
        {
            50
        } else {
            200
        };
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }
    // Al cerrar editando se guarda (en el chat, nunca en el archivo); y lo
    // anotado con el lapiz, junto al libro.
    est.dejar_de_anotar();
    est.dejar_de_editar(motor);
}

/// Un fotograma, ya.
fn pintar_ya(superficie: &Superficie, motor: &MotorRender, est: &mut Estado, textos: &Catalogo) {
    preparar(est, motor);
    if let Ok(destino) = superficie.empezar(motor) {
        let _ = motor.dibujar(&destino, |p: &Pintor| pintar(p, est, textos));
        let _ = superficie.presentar();
    }
}

/// **Lo que hace una tecla en el visor** (viendo, y editando lo que no es de
/// editar: moverse, el zoom, las hojas). `false` si cierra la ventana.
fn tecla_del_visor(
    est: &mut Estado,
    pin: &mut PinLector,
    ventana: &mut VentanaOverlay,
    (vk, shift, ctrl, alt): (u32, bool, bool, bool),
    textos: &Catalogo,
    motor: &MotorRender,
) -> bool {
    match logica::accion_de_tecla(vk, shift, ctrl, alt) {
        Some(AccionTecla::Mover(m, ext)) => est.mover(m, ext),
        Some(AccionTecla::HojaAnterior) => {
            if let Some(i) = est.hoja_vecina(false) {
                est.cambiar_hoja(i, motor, textos);
            }
        }
        Some(AccionTecla::HojaSiguiente) => {
            if let Some(i) = est.hoja_vecina(true) {
                est.cambiar_hoja(i, motor, textos);
            }
        }
        Some(AccionTecla::Copiar) => est.copiar(textos),
        Some(AccionTecla::SeleccionarTodo) => {
            let u = ultima(&est.abierta().hoja);
            est.abierta_mut().sel = Seleccion {
                ancla: (0, 0),
                activa: u,
            };
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
        Some(AccionTecla::PantallaCompleta) => pin.alternar_pantalla_completa(ventana),
        Some(AccionTecla::AbrirEnlace) => {
            if !est.editando {
                let a = est.abierta().sel.activa;
                est.abrir_enlace(a, textos, motor);
            }
        }
        Some(AccionTecla::Escape) => {
            if pin.pantalla_completa {
                pin.alternar_pantalla_completa(ventana);
            } else {
                return false;
            }
        }
        None => {}
    }
    true
}

/// Los botones de la barra en modo ver: «Editar» si la hoja es del chat y,
/// si hay archivo, «Abrir con…». Sin «Copiar»: Ctrl+C copia.
fn botones_de_ver(editable: bool, con_archivo: bool) -> Vec<BotonLector> {
    let mut b = Vec::with_capacity(2);
    if editable {
        b.push(BotonLector {
            id: BOTON_EDITAR,
            icono: &material::EDIT_NOTE,
            encendido: false,
        });
    }
    if con_archivo {
        b.push(BotonLector {
            id: BOTON_ABRIR_CON,
            icono: &material::OPEN_IN_NEW,
            encendido: false,
        });
    }
    b
}

/// El nombre y la tecla de los botones de [`botones_de_ver`], para el globo
/// que sale con el raton encima.
fn pistas_de_ver(textos: &Catalogo) -> Vec<(u8, String)> {
    vec![
        (BOTON_EDITAR, textos.t("hoja-pista-editar")),
        (BOTON_ABRIR_CON, textos.t("abrir-con")),
    ]
}

/// **Exportar el libro** con el componente comun (ver
/// `compartir::exportable::de_libro`). Sin fichero (una tabla del chat) no
/// hay nada que leer aqui.
fn exportar_libro(ruta: Option<&Path>) {
    let (Some(r), Some(u)) = (ruta, crate::oscuro::ubicacion()) else {
        return;
    };
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(r));
    match crate::compartir::exportable::de_libro(&titulo, r) {
        Ok(mut x) => {
            x.raiz = Some(u.raiz().to_path_buf());
            crate::compartir::ventana::abrir(
                crate::compartir::idioma_de(&u),
                u,
                crate::compartir::Cosa::Visor(x),
            );
        }
        Err(e) => tracing::warn!(?e, "no se pudo preparar el libro para exportar"),
    }
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
    let sin_interfaz = est.sin_interfaz;
    let Estado {
        libro,
        hojas,
        activa,
        cache,
        ..
    } = est;
    let libro: &Libro = libro;
    let Some(ab) = hojas[*activa].as_mut() else {
        return;
    };
    let (v, celdas) = vista_de(ab, caja);
    let paneles = ab.rejilla.paneles(&vista_celdas(&v, celdas));
    for p in paneles {
        hoja_office::autoajustar_filas(
            cache,
            motor,
            libro,
            &ab.hoja,
            &mut ab.rejilla,
            p.fila0,
            p.fila1 + 1,
        );
    }
    // Para la foto de un marco la vista va justo donde se puso (centrada en
    // el marco aunque se pase de lo usado).
    if !sin_interfaz {
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
    }
}

fn texto_centrado_v(
    p: &Pintor,
    texto: &str,
    x: f32,
    caja: RectF,
    tam: f32,
    ancho: f32,
    color: Color,
) {
    let (_, h) = p.medir_texto("Ag", tam);
    p.texto_linea(
        texto,
        x,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        ancho.max(0.0),
        color,
    );
}

/// El texto de la tarjeta de un comentario: el de una nota tal cual; el de un
/// hilo, con el nombre de cada quien encima de lo que dijo.
fn texto_de_comentario(c: &pixpin_hojas::comentarios::Comentario) -> String {
    if !c.encadenado {
        return c.texto.clone();
    }
    let mut s = format!("{}\n{}", c.autor, c.texto);
    for (autor, texto) in &c.respuestas {
        s.push_str(&format!("\n\n{autor}\n{texto}"));
    }
    s
}

/// **La tarjeta de un comentario** al pasar el raton por su celda: a la
/// derecha de la celda (a la izquierda si no cabe), amarillo palido como la
/// nota de Excel.
fn tarjeta_de_comentario(
    p: &Pintor,
    ab: &Abierta,
    v: &Vista,
    celdas: RectF,
    area: RectF,
    celda: (u32, u32),
    e: f32,
) {
    let Some(c) = ab.hoja.comentario_en(celda.0, celda.1) else {
        return;
    };
    let rango = ab.hoja.combinada_en(celda.0, celda.1).unwrap_or(Rango {
        fila0: celda.0,
        col0: celda.1,
        fila1: celda.0,
        col1: celda.1,
    });
    let vc = vista_celdas(v, celdas);
    let Some(caja) = hoja_office::caja_de_celda(&ab.rejilla, &vc, celdas, rango.fila0, rango.col0)
    else {
        return;
    };
    let derecha = hoja_office::caja_de_celda(&ab.rejilla, &vc, celdas, rango.fila0, rango.col1)
        .map_or(caja[2], |c| c[2]);
    let texto = texto_de_comentario(c);
    let tam = TAM_TEXTO * e;
    let (w, h) = p.medir_texto_ajustado(&texto, tam, 280.0 * e);
    let (ancho, alto) = (w + 16.0 * e, h + 12.0 * e);
    let mut x = derecha + 6.0 * e;
    if x + ancho > area.x + area.ancho {
        x = (caja[0] - ancho - 6.0 * e).max(area.x);
    }
    let y = caja[1].min(area.y + area.alto - alto).max(area.y);
    let r = RectF { x, y, ancho, alto };
    let k = |c: Color| pixpin_render::oscuro::si(crate::oscuro::activo(), c);
    p.rellenar(
        r,
        k(Color {
            r: 1.0,
            g: 1.0,
            b: 0.88,
            a: 1.0,
        }),
    );
    p.trazar(
        r,
        1.0,
        k(Color {
            r: 0.45,
            g: 0.45,
            b: 0.45,
            a: 1.0,
        }),
    );
    p.texto_ajustado(&texto, x + 8.0 * e, y + 6.0 * e, tam, 280.0 * e, k(TEXTO));
}

fn pintar(p: &Pintor, est: &mut Estado, textos: &Catalogo) {
    let z = est.zonas();
    let (e, ancho) = (est.e, est.ancho);
    // Modo oscuro (luna/sol): el marco con la luz invertida; la hoja, en
    // su pintor (`OpcionesHoja::oscuro`). Editando, igual. La foto de un
    // marco va con sus colores de verdad (el papel no es negro).
    let oscuro = crate::oscuro::activo() && !est.sin_interfaz;
    let k = |c: Color| pixpin_render::oscuro::si(oscuro, c);
    p.limpiar(k(Color::BLANCO));
    let con_nota = if est.gesto.is_none() && !est.editando {
        est.comentario_bajo
    } else {
        None
    };
    // Editando: lo que se escribe (o lo escrito tal cual, la formula) va en
    // la barra de formulas, y lo tecleado tambien encima de su celda.
    let editando = est.editando;
    let en_la_barra = est
        .editor()
        .filter(|_| editando)
        .map(|ed| (ed.en_la_barra().to_string(), ed.edicion.clone()));
    let anotando = est.anotando;
    let sin_interfaz = est.sin_interfaz;
    let con_nota = con_nota.filter(|_| !sin_interfaz);
    let Estado {
        libro,
        hojas,
        activa,
        cache,
        desde_pestana,
        encima_de,
        pestanas,
        bajo_raton,
        anotado,
        ..
    } = est;
    let libro: &Libro = libro;
    let activa = *activa;
    let Some(ab) = hojas[activa].as_ref() else {
        return;
    };

    // La hoja.
    let (v, celdas) = vista_de(ab, z.hoja);
    let op = if sin_interfaz {
        OpcionesHoja {
            encabezados: true,
            seleccion: None,
            activa: None,
            bajo_raton: None,
            oscuro,
        }
    } else {
        OpcionesHoja {
            encabezados: true,
            seleccion: Some(ab.sel.rango()),
            activa: Some(ab.sel.activa),
            bajo_raton: *bajo_raton,
            oscuro,
        }
    };
    hoja_office::pintar_hoja(p, cache, libro, &ab.hoja, &ab.rejilla, &v, z.hoja, &op);
    // Lo anotado (el asistente y el lapiz), encima de las celdas.
    anotado.pintar(
        p,
        libro,
        &ab.hoja,
        &ab.rejilla,
        &v,
        celdas,
        oscuro,
        anotando && !sin_interfaz,
    );
    if let Some(celda) = con_nota {
        tarjeta_de_comentario(p, ab, &v, celdas, z.hoja, celda, e);
    }
    if let Some((_, Some(tecleado))) = &en_la_barra
        && let Some(caja) = hoja_office::caja_de_celda(
            &ab.rejilla,
            &vista_celdas(&v, celdas),
            celdas,
            ab.sel.activa.0,
            ab.sel.activa.1,
        )
    {
        // Calibri 11, la letra de las tablas, al zoom que se ve.
        let tam = (11.0 * ab.rejilla.ppp / 72.0 * ab.zoom) as f32;
        crate::hoja_pin::pintar_escritura(p, tecleado, caja, tam, celdas);
    }

    // Las barras de desplazamiento.
    let (bv, bh) = barras(ab, celdas, e, *encima_de);
    for (eje, b) in [(Eje::Vertical, bv), (Eje::Horizontal, bh)] {
        if let Some(b) = b.filter(|_| !sin_interfaz) {
            let color = k(if *encima_de == Some(eje) {
                PULGAR_ENCIMA
            } else {
                PULGAR
            });
            let r = b.pulgar.ancho.min(b.pulgar.alto) / 2.0;
            p.rellenar_redondeado(b.pulgar, r, color);
        }
    }

    // La barra de formulas: la celda activa y lo que tiene.
    let f = z.formula;
    p.rellenar(f, k(FONDO_BARRA));
    p.rellenar(
        RectF {
            y: f.y + f.alto - 1.0,
            alto: 1.0,
            ..f
        },
        k(LINEA),
    );
    let tam = TAM_TEXTO * e;
    let (fa, ca) = ab.sel.activa;
    let r = ab.sel.rango();
    let referencia = if r.fila0 == r.fila1 && r.col0 == r.col1 {
        format!("{}{}", nombre_columna(ca), fa + 1)
    } else {
        format!(
            "{}{}:{}{}",
            nombre_columna(r.col0),
            r.fila0 + 1,
            nombre_columna(r.col1),
            r.fila1 + 1
        )
    };
    let ancho_ref = 150.0 * e;
    texto_centrado_v(
        p,
        &referencia,
        f.x + 8.0 * e,
        f,
        tam,
        ancho_ref - 12.0 * e,
        k(TEXTO),
    );
    p.rellenar(
        RectF {
            x: f.x + ancho_ref,
            y: f.y + 5.0 * e,
            ancho: 1.0,
            alto: f.alto - 10.0 * e,
        },
        k(LINEA),
    );
    let x0 = f.x + ancho_ref + 10.0 * e;
    if let Some((contenido, tecleado)) = &en_la_barra {
        // A la derecha, que se esta editando (en el verde de Excel).
        let modo = textos.t("hoja-editando");
        let ancho_modo = p.medir_texto(&modo, tam).0 + 20.0 * e;
        let una_linea = contenido.replace(['\r', '\n'], " ⏎ ");
        let cabe = f.x + f.ancho - x0 - ancho_modo - 10.0 * e;
        texto_centrado_v(p, &una_linea, x0, f, tam, cabe, k(TEXTO));
        if tecleado.is_some() {
            let cx = (x0 + p.medir_texto(&una_linea, tam).0 + 1.0).min(x0 + cabe);
            p.rellenar(
                RectF {
                    x: cx,
                    y: f.y + 6.0 * e,
                    ancho: e.max(1.0),
                    alto: f.alto - 12.0 * e,
                },
                k(TEXTO),
            );
        }
        texto_centrado_v(
            p,
            &modo,
            f.x + f.ancho - ancho_modo + 10.0 * e,
            f,
            tam,
            ancho_modo,
            k(VERDE),
        );
    } else {
        let celda = ab.hoja.celda(fa, ca);
        let contenido = celda
            .and_then(|c| libro.contenido(&ab.hoja, c))
            .map(|c| c.mostrado.texto)
            .unwrap_or_default();
        let enlace = ab.hoja.hipervinculo_en(fa, ca).map(|h| h.destino.clone());
        let ancho_enlace = if enlace.is_some() {
            (f.ancho * 0.35).min(360.0 * e)
        } else {
            0.0
        };
        let una_linea = contenido.replace(['\r', '\n'], " ⏎ ");
        texto_centrado_v(
            p,
            &una_linea,
            x0,
            f,
            tam,
            f.x + f.ancho - x0 - ancho_enlace - 10.0 * e,
            k(TEXTO),
        );
        if let Some(d) = enlace {
            texto_centrado_v(
                p,
                &d,
                f.x + f.ancho - ancho_enlace - 6.0 * e,
                f,
                tam,
                ancho_enlace,
                k(ENLACE),
            );
        }
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
    let ancho_estado = if estado.is_empty() {
        0.0
    } else {
        p.medir_texto(&estado, tam).0 + 24.0 * e
    };
    let tira = RectF {
        ancho: (z.pestanas.ancho - ancho_estado).max(0.0),
        ..z.pestanas
    };
    hoja_office::poner_oscuro(oscuro);
    *pestanas = hoja_office::pintar_pestanas(p, cache, libro, activa, *desde_pestana, tira, e);
    if ancho_estado > 0.0 {
        let caja = RectF {
            x: z.pestanas.x + z.pestanas.ancho - ancho_estado,
            ancho: ancho_estado,
            ..z.pestanas
        };
        p.rellenar(
            caja,
            k(Color {
                r: 0.953,
                g: 0.953,
                b: 0.953,
                a: 1.0,
            }),
        );
        p.rellenar(RectF { alto: 1.0, ..caja }, k(LINEA));
        texto_centrado_v(
            p,
            &estado,
            caja.x + 12.0 * e,
            caja,
            tam,
            ancho_estado - 16.0 * e,
            k(TENUE),
        );
    }

    if sin_interfaz {
        return;
    }
    // Los marcos para imprimir, con su numero, y el riel (y anotando, la
    // barra del lapiz), encima de la hoja.
    est.pintar_marcos(p, &z, textos);
    riel::pintar(p, est, textos);

    if let Some((texto, _)) = &est.aviso {
        crate::lector::aviso_en(p, texto, ancho / 2.0, z.pestanas.y - 56.0 * e, e);
    }
}

#[cfg(test)]
mod pruebas;
