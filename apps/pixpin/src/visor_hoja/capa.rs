//! **La capa de anotaciones de una hoja de calculo** (15-oct-2026): lo que
//! el asistente anota en un Excel (`asistente::anotar_inteligente`) va a
//! una capa de tinta por hoja junto al libro
//! (`<libro>.pixpin-anotado/hoja-<n>.excalidraw`, la misma forma que la de
//! un Word o un PDF), y aqui se pinta encima de las celdas con el motor
//! comun (`lector_tinta::Tinta`), se relee al cambiar en el disco (enseguida
//! con el aviso del asistente) y Ctrl+Z / Ctrl+Y (viendo, no editando)
//! quitan y devuelven el ultimo turno del asistente.
//!
//! **Las unidades**: la capa va en pixeles de la hoja al 100 % a 96 ppp con
//! el ancho de digito de la tabla (`anotar_inteligente::rejilla_de_la_capa`,
//! sin DirectWrite: lo puede calcular el asistente sin ventana). El visor
//! mide la letra de verdad y a sus ppp, asi que cada punto se lleva a su
//! celda: la misma fraccion de la misma celda en la rejilla de aqui. Lo
//! anotado cae siempre en su celda aunque las columnas midan algo distinto.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use pixpin_hojas::{Hoja, Libro, Rejilla, Vista};
use pixpin_motor2d::{Elemento, Figura};
use pixpin_render::{Color, Pintor, RectF};

use crate::lector_tinta::{self, Capa, Tinta};

/// Cada cuanto, como mucho, se mira si el disco cambio.
const MIRAR: Duration = Duration::from_millis(700);

type Sello = Option<(SystemTime, u64)>;

fn sello_de(r: &Path) -> Sello {
    std::fs::metadata(r).ok().and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// La capa de la hoja abierta, leida y llevada a la rejilla del visor.
#[derive(Default)]
pub(super) struct Anotado {
    /// La hoja de la que es (indice en el libro) y su fichero.
    hoja: Option<usize>,
    ruta: PathBuf,
    sello: Sello,
    /// Como esta en el disco (en las unidades de la capa).
    capa: Option<Capa>,
    /// La misma, en la rejilla del visor, y para que version de la hoja.
    vista: Option<(u64, Capa)>,
    tinta: Option<Tinta>,
    mirado: Option<Instant>,
    aviso: u64,
}

/// Lleva una x (o una y) de una rejilla a otra por su celda.
fn llevar_x(x: f32, de: &Rejilla, a: &Rejilla) -> f32 {
    let x = x as f64;
    let c = if x < 0.0 { 0 } else { de.col_en(x) };
    let ancho = de.ancho(c).max(1e-6);
    (a.x(c) + (x - de.x(c)) / ancho * a.ancho(c)) as f32
}

fn llevar_y(y: f32, de: &Rejilla, a: &Rejilla) -> f32 {
    let y = y as f64;
    let f = if y < 0.0 { 0 } else { de.fila_en(y) };
    let alto = de.alto(f).max(1e-6);
    (a.y(f) + (y - de.y(f)) / alto * a.alto(f)) as f32
}

/// **Un elemento de la capa, en la rejilla del visor**: sus esquinas y
/// puntos a su celda, y la letra y el grosor a la escala de alli.
pub(super) fn llevar(e: &Elemento, de: &Rejilla, a: &Rejilla) -> Elemento {
    let mut n = e.clone();
    let (x0, y0) = (llevar_x(e.x, de, a), llevar_y(e.y, de, a));
    let (x1, y1) = (llevar_x(e.x + e.ancho, de, a), llevar_y(e.y + e.alto, de, a));
    let f = if e.y < 0.0 { 0 } else { de.fila_en(e.y as f64) };
    let k = (a.alto(f) / de.alto(f).max(1e-6)) as f32;
    n.x = x0;
    n.y = y0;
    n.ancho = (x1 - x0).max(0.0);
    n.alto = (y1 - y0).max(0.0);
    n.grosor *= k;
    match &mut n.figura {
        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } | Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos } => {
            for p in puntos.iter_mut() {
                p.x = llevar_x(p.x, de, a);
                p.y = llevar_y(p.y, de, a);
            }
        }
        Figura::Texto { tam, .. } => {
            *tam *= k;
            // La caja del texto, de su tamano (no estirada por la celda).
            n.ancho = e.ancho * k;
            n.alto = e.alto * k;
        }
        _ => {}
    }
    n
}

impl Anotado {
    /// **Mira si hay que (re)leer la capa** de la hoja `hoja` del libro
    /// `ruta`: otra hoja, el fichero cambio (como mucho cada 0,7 s) o el
    /// asistente aviso. Dice si cambio algo (hay que pintar).
    pub(super) fn mirar(&mut self, ruta: Option<&Path>, hoja: usize) -> bool {
        let Some(libro) = ruta else { return false };
        let avisado = lector_tinta::turnos::hay_aviso(&mut self.aviso);
        let otra = self.hoja != Some(hoja);
        if !otra && !avisado && self.mirado.is_some_and(|t| t.elapsed() < MIRAR) {
            return false;
        }
        self.mirado = Some(Instant::now());
        let r = lector_tinta::ruta_de_hoja(libro, hoja);
        let sello = sello_de(&r);
        if !otra && sello == self.sello {
            return false;
        }
        // Con algo deshecho sin guardar, lo de aqui manda.
        if !otra && self.capa.as_ref().is_some_and(|c| c.sucia) {
            return false;
        }
        self.hoja = Some(hoja);
        self.sello = sello;
        self.capa = sello.is_some().then(|| Capa::leer(&r)).filter(|c| !c.vacia() || c.turno.is_some());
        self.ruta = r;
        self.vista = None;
        if let Some(t) = self.tinta.as_mut() {
            t.olvidar_lo_calculado();
        }
        true
    }

    /// **Ctrl+Z**: quita el ultimo turno del asistente (si es lo ultimo).
    pub(super) fn deshacer(&mut self) -> bool {
        let Some(c) = self.capa.as_mut() else { return false };
        if !c.turno.as_ref().is_some_and(|t| t.arriba(&c.escena)) || !c.deshacer() {
            return false;
        }
        self.guardar();
        true
    }

    /// **Ctrl+Y**: lo devuelve.
    pub(super) fn rehacer(&mut self) -> bool {
        let Some(c) = self.capa.as_mut() else { return false };
        if !c.turno.as_ref().is_some_and(|t| t.deshecho(&c.escena)) || !c.rehacer() {
            return false;
        }
        self.guardar();
        true
    }

    fn guardar(&mut self) {
        if let Some(c) = self.capa.as_mut() {
            if let Err(e) = c.guardar(&self.ruta) {
                tracing::warn!(?e, "visor de hojas: no se pudo guardar lo anotado");
            }
            self.sello = sello_de(&self.ruta);
        }
        self.vista = None;
    }

    /// **Pinta la capa** sobre las celdas: `real`, la rejilla del visor;
    /// `celdas` y `v`, donde y como se ve la hoja.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pintar(&mut self, p: &Pintor, libro: &Libro, hoja: &Hoja, real: &Rejilla, v: &Vista, celdas: RectF, oscuro: bool) {
        let Some(capa) = self.capa.as_ref() else { return };
        if capa.vacia() {
            return;
        }
        // La capa en la rejilla del visor (se rehace si llegaron filas).
        let clave = hoja.version ^ (real.x(1).to_bits() << 1);
        if self.vista.as_ref().is_none_or(|(k, _)| *k != clave) {
            let de = crate::asistente::anotar_inteligente::rejilla_de_la_capa(libro, hoja);
            let mut vista = Capa::default();
            for e in capa.escena.visibles() {
                vista.escena.anadir(llevar(e, &de, real));
            }
            self.vista = Some((clave, vista));
            if let Some(t) = self.tinta.as_mut() {
                t.olvidar_lo_calculado();
            }
        }
        let Some((_, vista)) = self.vista.as_ref() else { return };
        // El panel que se mueve (el ultimo): ahi va lo anotado.
        let vc = Vista { ancho: celdas.ancho as f64, alto: celdas.alto as f64, ..*v };
        let Some(panel) = real.paneles(&vc).last().copied() else { return };
        let e = v.escala.max(0.05) as f32;
        let [px, py, pw, ph] = panel.pantalla.map(|x| x as f32);
        let caja = RectF { x: celdas.x + px, y: celdas.y + py, ancho: pw, alto: ph };
        let (ox, oy) = (panel.origen.0 as f32, panel.origen.1 as f32);
        let tinta = self.tinta.get_or_insert_with(Tinta::nueva);
        let papel = if oscuro { pixpin_render::oscuro::PAPEL } else { Color::BLANCO };
        p.con_recorte(caja, |p| {
            p.poner_vista((caja.x, caja.y), e, (-ox * e, -oy * e));
            tinta.pintar_capa(p, 0, vista, (ox, oy, ox + pw / e, oy + ph / e), e, lector_tinta::papel(papel), false);
            p.desplazar(0.0, 0.0);
        });
    }
}
