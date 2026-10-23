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
//!
//! **El lapiz del riel** (19-oct-2026, el usuario: «el lapiz, el marcador y
//! el marco deben ser universales»): la misma capa se anota a mano con las
//! herramientas del lienzo (`lector_tinta::Tinta`, como en el PDF y el
//! Word). Se dibuja sobre la capa ya llevada a la rejilla del visor (la
//! `vista`, con los mismos ids que la del disco) y, al acabar cada gesto, lo
//! tocado vuelve a las unidades de la capa y se escribe en el mismo fichero
//! (`hoja-<n>.excalidraw`); lo que no se toco vuelve tal cual del disco.
//!
//! **La de un libro adjunto al chat viaja** (20-oct-2026): va en
//! `anot-<uid>-h<i>` de su mensaje (`i` desde 0; en el PC
//! `lienzos/anot-<uid>-h<i>.excalidraw`), con la de junto al libro pasada la
//! primera vez (`anotado_del_adjunto::tinta_de_la_hoja_del_libro`). El
//! asistente escribe en la misma.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use pixpin_geom::Rect;
use pixpin_hojas::{Hoja, Libro, Rejilla, Vista};
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::{Elemento, Figura};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_shell::overlay::EventoOverlay;

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
    /// De cada elemento de la `vista`: como quedo en ella y como esta en la
    /// capa. Al devolver lo anotado, lo que no cambio vuelve tal cual (ir y
    /// volver por la rejilla en `f32` no siempre deja el mismo numero).
    originales: HashMap<u64, (Elemento, Elemento)>,
}

/// El panel que se mueve (el ultimo, donde va lo anotado): su caja en la
/// ventana y el punto de la hoja de su esquina.
fn panel_que_se_mueve(real: &Rejilla, v: &Vista, celdas: RectF) -> Option<(RectF, (f32, f32))> {
    let vc = Vista { ancho: celdas.ancho as f64, alto: celdas.alto as f64, ..*v };
    let panel = real.paneles(&vc).last().copied()?;
    let [px, py, pw, ph] = panel.pantalla.map(|x| x as f32);
    Some((RectF { x: celdas.x + px, y: celdas.y + py, ancho: pw, alto: ph }, (panel.origen.0 as f32, panel.origen.1 as f32)))
}

/// **La camara de la tinta**: la misma transformada con la que se pinta la
/// capa en el panel que se mueve, para que el raton caiga donde se ve
/// (pixeles de ventana a pixeles de la hoja al 100 %).
pub(super) fn camara(real: &Rejilla, v: &Vista, celdas: RectF) -> Camara {
    let z = v.escala.max(0.05) as f32;
    let (caja, (ox, oy)) = panel_que_se_mueve(real, v, celdas).unwrap_or((celdas, (v.x as f32, v.y as f32)));
    Camara { x: ox - caja.x / z, y: oy - caja.y / z, zoom: z }
}

/// **Cuando hay que volver a llevar la capa a la rejilla**: otra version de
/// la hoja, otro ancho de letra o filas que cambiaron de alto (el autoajuste
/// de las filas con texto ajustado, que se mide al irlas viendo: sin esto lo
/// anotado debajo de ellas se quedaba donde estaba y no en su celda).
pub(super) fn clave_de(hoja: &Hoja, real: &Rejilla) -> u64 {
    hoja.version ^ (real.x(1).to_bits() << 1) ^ real.y(u32::MAX).to_bits().rotate_left(23)
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
        // Donde esta se busca al cambiar de hoja (de un adjunto del chat, la
        // del mensaje, que viaja: `anot-<uid>-h<n>`); luego, la misma.
        let r = if otra || self.ruta.as_os_str().is_empty() { crate::anotado_del_adjunto::tinta_de_la_hoja_del_libro(libro, hoja) } else { self.ruta.clone() };
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

    /// **La capa en la rejilla del visor**, rehecha si cambio la hoja (llegaron
    /// filas, otro ancho de letra). Conserva los ids y la historia de la capa
    /// (lo que elige la tinta y el deshacer siguen valiendo).
    fn asegurar_vista(&mut self, libro: &Libro, hoja: &Hoja, real: &Rejilla) {
        let clave = clave_de(hoja, real);
        if self.vista.as_ref().is_some_and(|(k, _)| *k == clave) {
            return;
        }
        let Some(capa) = self.capa.as_ref() else {
            self.vista = None;
            return;
        };
        let de = crate::asistente::anotar_inteligente::rejilla_de_la_capa(libro, hoja);
        let mut vista = Capa::default();
        vista.escena = capa.escena.clone();
        self.originales.clear();
        for e in vista.escena.elementos.iter_mut() {
            let original = e.clone();
            *e = llevar(&original, &de, real);
            self.originales.insert(e.id, (e.clone(), original));
        }
        self.vista = Some((clave, vista));
        if let Some(t) = self.tinta.as_mut() {
            t.olvidar_lo_calculado();
        }
    }

    /// Que haya capa (vacia si no hay nada anotado) y su vista, de la hoja
    /// `i`.
    fn preparar(&mut self, libro: &Libro, hoja: &Hoja, i: usize, real: &Rejilla) {
        // Sin fichero (una tabla del chat sin libro), la capa es de la sesion
        // y de una hoja: otra hoja empieza en blanco.
        if self.hoja != Some(i) && self.ruta.as_os_str().is_empty() {
            self.hoja = Some(i);
            self.capa = None;
            self.vista = None;
        }
        if self.capa.is_none() {
            self.capa = Some(Capa::default());
            self.vista = None;
        }
        self.asegurar_vista(libro, hoja, real);
    }

    /// **Un evento para el lapiz del riel** (`lector_tinta::Tinta`) en la hoja
    /// `i` del libro, con la camara del panel que se mueve.
    pub(super) fn evento(&mut self, ev: &EventoOverlay, (libro, hoja, i): (&Libro, &Hoja, usize), real: &Rejilla, camara: &Camara, (area, escala_por_cien): (Rect, u32)) -> lector_tinta::Hecho {
        self.preparar(libro, hoja, i, real);
        let tinta = self.tinta.get_or_insert_with(Tinta::nueva);
        tinta.a_la_hoja(0);
        let Some((_, vista)) = self.vista.as_mut() else { return lector_tinta::Hecho::default() };
        tinta.evento(ev, vista, camara, area, escala_por_cien)
    }

    /// **El gesto del lapiz acabo**: lo de la vista vuelve a la capa (lo
    /// tocado, de la rejilla del visor a la de la capa; lo demas, tal cual
    /// estaba) y se escribe.
    pub(super) fn devolver(&mut self, libro: &Libro, hoja: &Hoja, real: &Rejilla) {
        let (Some(capa), Some((_, vista))) = (self.capa.as_mut(), self.vista.as_ref()) else { return };
        let de = crate::asistente::anotar_inteligente::rejilla_de_la_capa(libro, hoja);
        let mut escena = vista.escena.clone();
        let mut originales = HashMap::with_capacity(escena.elementos.len());
        for e in escena.elementos.iter_mut() {
            let en_la_vista = e.clone();
            *e = match self.originales.get(&e.id) {
                Some((m, o)) if *m == en_la_vista => o.clone(),
                // `llevar` es la misma cuenta al reves: celda a celda.
                _ => llevar(&en_la_vista, real, &de),
            };
            originales.insert(e.id, (en_la_vista, e.clone()));
        }
        capa.escena = escena;
        capa.sucia = true;
        self.originales = originales;
        if self.ruta.as_os_str().is_empty() {
            capa.sucia = false;
            return;
        }
        if let Err(e) = capa.guardar(&self.ruta) {
            tracing::warn!(?e, "visor de hojas: no se pudo guardar lo anotado");
        }
        self.sello = sello_de(&self.ruta);
    }

    /// **Ctrl+Z / Ctrl+Y del lapiz**: en la vista, y de vuelta a la capa.
    pub(super) fn deshacer_lo_mio(&mut self, libro: &Libro, hoja: &Hoja, real: &Rejilla, rehacer: bool) -> bool {
        let Some((_, vista)) = self.vista.as_mut() else { return false };
        let hecho = if rehacer { vista.rehacer() } else { vista.deshacer() };
        if hecho {
            self.devolver(libro, hoja, real);
        }
        hecho
    }

    /// El gesto de pararse: el trazo quieto se vuelve figura.
    pub(super) fn forma_rapida(&mut self, zoom: f32) -> bool {
        let (Some(t), Some((_, vista))) = (self.tinta.as_mut(), self.vista.as_mut()) else { return false };
        t.forma_rapida(vista, zoom)
    }

    /// La tinta, si ya se uso (para saber si quiere Escape o esta trazando).
    pub(super) fn tinta(&self) -> Option<&Tinta> {
        self.tinta.as_ref()
    }

    /// La barra y el panel del lienzo, arriba (anotando).
    pub(super) fn pintar_interfaz(&mut self, p: &Pintor, area: Rect, escala_por_cien: u32) {
        let tinta = self.tinta.get_or_insert_with(Tinta::nueva);
        tinta.pintar_interfaz(p, self.vista.as_ref().map(|(_, c)| c), area, escala_por_cien);
    }

    /// **Pinta la capa** sobre las celdas: `real`, la rejilla del visor;
    /// `celdas` y `v`, donde y como se ve la hoja. `activa`: anotando (sale
    /// lo que se esta trazando y lo elegido).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pintar(&mut self, p: &Pintor, libro: &Libro, hoja: &Hoja, real: &Rejilla, v: &Vista, celdas: RectF, oscuro: bool, activa: bool) {
        if self.capa.as_ref().is_none_or(|c| c.vacia()) && !activa {
            return;
        }
        // La capa en la rejilla del visor (se rehace si llegaron filas).
        self.asegurar_vista(libro, hoja, real);
        let Some((_, vista)) = self.vista.as_ref() else { return };
        let Some((caja, (ox, oy))) = panel_que_se_mueve(real, v, celdas) else { return };
        let e = v.escala.max(0.05) as f32;
        let tinta = self.tinta.get_or_insert_with(Tinta::nueva);
        let papel = if oscuro { pixpin_render::oscuro::PAPEL } else { Color::BLANCO };
        p.con_recorte(caja, |p| {
            p.poner_vista((caja.x, caja.y), e, (-ox * e, -oy * e));
            tinta.pintar_capa(p, 0, vista, (ox, oy, ox + caja.ancho / e, oy + caja.alto / e), e, lector_tinta::papel(papel), activa);
            p.desplazar(0.0, 0.0);
        });
    }
}
