//! **Un diagrama o una figura geometrica, a figuras EXACTAS del lienzo.**
//!
//! Queja del usuario (15-oct-2026, con captura): pego en el lienzo una
//! lamina de geometria (circunferencia, cuadrante relleno, cuerdas, arcos de
//! color, puntos A B C D O P, marcas de angulo, rectas punteadas con flecha,
//! recuadros, formulas) y pidio «calca esto». El calco por manchas de color
//! (`calcar`) la dejo en borrones: sin rayas finas, el titulo en garabatos,
//! la circunferencia perdida. Pidio «dibujalo usando las herramientas
//! correctas, no solo lapiz».
//!
//! Aqui la geometria **la mide Rust**, sin modelo y en decenas de ms:
//!
//! 1. **Mascaras** ([`mascara`]): el papel, la tinta, sus colores (k-medias
//!    con el antialias atribuido a su color), lo que es **relleno** (lo que
//!    sobrevive a una erosion: regiones, puntos, puntas de flecha) y lo que
//!    es **trazo** (lo fino). El texto que leyo el OCR se aparta antes.
//! 2. **Esqueleto** de los trazos (Zhang-Suen) y sus **cadenas** de cruce a
//!    cruce, troceadas en tramos rectos y de arco ([`ajuste`]: recta de
//!    minimo error perpendicular; circunferencia de Kasa refinada con
//!    Gauss-Newton).
//! 3. **Juntar** ([`geometria`]): los tramos colineales en un segmento (con
//!    su grosor y color medidos, y discontinuo o punteado si el perfil de
//!    tinta tiene huecos regulares); los arcos de la misma circunferencia en
//!    una (entera si da la vuelta) o en arcos con sus extremos, partidos
//!    donde cambia el color (el arco verde de una circunferencia negra).
//! 4. **Puntos** (circulos rellenos pequenos), **puntas de flecha**,
//!    **marcas de angulo** y **regiones** de color (con su color medio; un
//!    degradado queda en bandas), **recuadros** (cuatro segmentos en
//!    rectangulo) y los **rotulos**: el texto del OCR y las letras sueltas
//!    que el OCR no vio, leidas aparte ([`Lector`]).
//! 5. **Topologia**: los extremos de los segmentos se pegan a los puntos, a
//!    la circunferencia y a los cruces con otras rectas.
//!
//! La nube (con vision) solo interpreta lo dudoso —las etiquetas que el OCR
//! no leyo bien— despues, sin frenar lo primero que se ve
//! (`hoja::vision::pedir_rotulos`).
//!
//! Lo usan `hoja::montaje` (zonas de dibujo que resultan diagrama) y el
//! calco (`calcar_imagen` con una lamina con texto, en la app).

pub mod ajuste;
mod geometria;
pub mod mascara;
pub mod restricciones;
mod salida;

#[cfg(test)]
mod pruebas;

use std::time::Instant;

use crate::hoja::{Caja, PalabraHoja};
use mascara::{Img, P};

pub(crate) use salida::elementos;
pub use salida::a_elementos;

/// Como va un trazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Estilo {
    #[default]
    Solido,
    Discontinuo,
    Punteado,
}

/// Un segmento recto (o flecha).
#[derive(Debug, Clone, PartialEq)]
pub struct Segmento {
    pub a: P,
    pub b: P,
    pub grosor: f64,
    pub color: [u8; 3],
    pub estilo: Estilo,
    pub flecha_a: bool,
    pub flecha_b: bool,
}

impl Segmento {
    pub fn largo(&self) -> f64 {
        ajuste::dist(self.a, self.b)
    }
}

/// Una circunferencia o un arco suyo. `inicio` y `barrido` en radianes de
/// la pantalla (la y baja: un barrido positivo va en el sentido del reloj);
/// `barrido >= 2 pi` es la circunferencia entera.
#[derive(Debug, Clone, PartialEq)]
pub struct Arco {
    pub c: P,
    pub r: f64,
    pub inicio: f64,
    pub barrido: f64,
    pub grosor: f64,
    pub color: [u8; 3],
    pub estilo: Estilo,
}

impl Arco {
    pub fn entera(&self) -> bool {
        self.barrido >= std::f64::consts::TAU - 1e-6
    }
    pub fn extremos(&self) -> (P, P) {
        let a = (self.c.0 + self.r * self.inicio.cos(), self.c.1 + self.r * self.inicio.sin());
        let f = self.inicio + self.barrido;
        (a, (self.c.0 + self.r * f.cos(), self.c.1 + self.r * f.sin()))
    }
}

/// Un punto (circulito relleno).
#[derive(Debug, Clone, PartialEq)]
pub struct Punto {
    pub c: P,
    pub r: f64,
    pub color: [u8; 3],
}

/// Una region rellena (con agujeros).
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub contorno: Vec<P>,
    pub huecos: Vec<Vec<P>>,
    pub color: [u8; 3],
    pub area: f64,
}

/// Un rectangulo de cuatro segmentos.
#[derive(Debug, Clone, PartialEq)]
pub struct Recuadro {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub grosor: f64,
    pub color: [u8; 3],
}

/// Un texto del diagrama (del OCR o una letra suelta leida aparte).
#[derive(Debug, Clone, PartialEq)]
pub struct Rotulo {
    pub texto: String,
    pub caja: Caja,
    pub color: Option<[u8; 3]>,
    pub negrita: bool,
    /// Una letra suelta (su caja es el alto de la mayuscula, no el renglon).
    pub letra_suelta: bool,
}

/// Una mancha pequena que parece una letra que el OCR no leyo: se lee
/// aparte; si nada la lee, sus trazos (`trazos`) van como lineas.
#[derive(Debug, Clone, PartialEq)]
pub struct Glifo {
    pub caja: Caja,
    pub color: [u8; 3],
    pub trazos: Vec<Vec<P>>,
    pub grosor: f64,
    /// Sus pixeles (x, y) en el recorte: solo ellos van a la hoja de letras.
    pub pixeles: Vec<(u32, u32)>,
}

/// **El diagrama medido**, en pixeles del recorte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diagrama {
    pub ancho: usize,
    pub alto: usize,
    pub segmentos: Vec<Segmento>,
    pub arcos: Vec<Arco>,
    pub puntos: Vec<Punto>,
    pub regiones: Vec<Region>,
    pub recuadros: Vec<Recuadro>,
    pub rotulos: Vec<Rotulo>,
    pub glifos: Vec<Glifo>,
    /// Que parte de la tinta (sin el texto) explican las figuras (0-1): un
    /// dibujo a mano o una foto se explican poco y van al calco.
    pub explica: f64,
    /// Cuanta tinta habia (pixeles, sin el texto).
    pub tinta: usize,
    /// Que parte de la tinta es relleno (regiones).
    pub de_relleno: f64,
    /// El largo mediano de los trazos (segmentos y arcos), en pixeles.
    pub largo_mediano: f64,
    /// Que parte del recorte es papel, y lo que se aparta del papel lo que
    /// lo es (el grano): una foto no tiene papel liso.
    pub papel: f64,
    pub grano: f64,
    pub ms: u128,
}

impl Diagrama {
    /// Cuantas figuras salieron (sin los rotulos).
    pub fn figuras(&self) -> usize {
        self.segmentos.len() + self.arcos.len() + self.puntos.len() + self.regiones.len() + self.recuadros.len()
    }
    /// **Si esto es de verdad un diagrama** (y no una ilustracion o una
    /// foto): la geometria explica casi toda la tinta, y hay geometria.
    pub fn es_diagrama(&self) -> bool {
        let lineas = self.segmentos.len() + self.arcos.len() + self.recuadros.len();
        // Una foto tambien «se explica» con sus manchas de color y mil
        // rayitas de ruido: un diagrama tiene trazos largos y no es todo relleno.
        let largo_min = 12f64.max(0.02 * self.ancho.max(self.alto) as f64);
        lineas >= 2 && self.explica >= UMBRAL_DIAGRAMA && self.largo_mediano >= largo_min && self.de_relleno <= 0.9 && self.papel >= 0.3 && self.grano <= 10.0
    }
    /// Lo que salio, en corto.
    pub fn resumen(&self) -> String {
        format!(
            "{} segmentos, {} arcos/circunferencias, {} puntos, {} regiones, {} recuadros, {} rotulos (explica {:.0} %, relleno {:.0} %, largo mediano {:.0} px, papel {:.0} % grano {:.1})",
            self.segmentos.len(),
            self.arcos.len(),
            self.puntos.len(),
            self.regiones.len(),
            self.recuadros.len(),
            self.rotulos.len() + self.glifos.len(),
            self.explica * 100.0,
            self.de_relleno * 100.0,
            self.largo_mediano,
            self.papel * 100.0,
            self.grano
        )
    }
}

/// Lo que tienen que explicar las figuras para que sea diagrama.
pub const UMBRAL_DIAGRAMA: f64 = 0.72;

/// **Quien lee las letras sueltas**: recibe una imagen RGBA (ancho, alto) y
/// devuelve las palabras con su caja en esa imagen (el OCR de Windows en la
/// app; uno de mentira en las pruebas).
pub type Lector = dyn Fn(&[u8], u32, u32) -> Vec<PalabraHoja> + Sync;

/// Las variables.
#[derive(Debug, Clone, PartialEq)]
pub struct Opciones {
    /// El alto de letra del texto de alrededor (pixeles del recorte).
    pub alto_letra: f64,
    /// Mas lejos del papel que esto es tinta (distancia de color RGB).
    pub umbral_tinta: f32,
    /// Texto de alrededor que no es del diagrama (de otras zonas de la hoja
    /// que caen en el recorte): no se vectoriza ni va como rotulo.
    pub tapar: Vec<Caja>,
    /// **El texto de la hoja** (renglones del OCR de alrededor, lo que dijo
    /// la nube): de aqui salen los hechos ([`restricciones::Hecho`]) que
    /// hacen exacta la geometria («AB es diametro», «cuadrante»).
    pub pistas: Vec<String>,
    /// Aplicar la geometria exacta ([`restricciones::aplicar`]).
    pub exacta: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones { alto_letra: 14.0, umbral_tinta: 34.0, tapar: Vec::new(), pistas: Vec::new(), exacta: true }
    }
}

/// **Mide el diagrama** de un recorte RGBA. `palabras`: lo que leyo el OCR
/// dentro (en pixeles del recorte), que va como rotulo y no se vectoriza.
/// `lector`: para las letras sueltas que el OCR no vio (sin el, van como
/// trazos). Con `o.exacta`, la geometria sale exacta figura a figura, con
/// los hechos de `o.pistas` y de sus propios rotulos.
pub fn vectorizar(rgba: &[u8], ancho: u32, alto: u32, palabras: &[PalabraHoja], o: &Opciones, lector: Option<&Lector>) -> Diagrama {
    let t0 = Instant::now();
    let (w, h) = (ancho as usize, alto as usize);
    if w < 4 || h < 4 || rgba.len() < w * h * 4 {
        return Diagrama { ancho: w, alto: h, ..Default::default() };
    }
    let img = Img::de_rgba(rgba, w, h);
    let mut d = geometria::medir(&img, palabras, o);
    if let Some(l) = lector {
        geometria::leer_glifos(&mut d, &img, l, o);
    }
    if o.exacta {
        exacta(&mut d, &o.pistas, o.alto_letra);
    }
    d.ms = t0.elapsed().as_millis();
    d
}

/// **La geometria exacta** ([`restricciones`]): los hechos de `pistas` y de
/// los rotulos del diagrama, figura a figura.
pub fn exacta(d: &mut Diagrama, pistas: &[String], alto_letra: f64) {
    let mut textos: Vec<String> = pistas.to_vec();
    textos.extend(d.rotulos.iter().map(|r| r.texto.clone()));
    let hechos = restricciones::hechos_de_texto(&textos);
    restricciones::aplicar(d, &hechos, alto_letra.max(6.0));
}

/// Los rotulos que dijo la nube (texto y su centro en fracciones 0-1 del
/// recorte) puestos en el diagrama: corrigen al OCR o a la letra suelta
/// que caen cerca, y lo que no cae cerca de nada se anade (si no pisa), con
/// el color de la tinta de alli si se pasa el recorte (`rgba`, ancho, alto).
pub fn corregir_rotulos(d: &mut Diagrama, de_la_nube: &[(String, f64, f64)], alto_letra: f64, recorte: Option<(&[u8], u32, u32)>) {
    let img = recorte.filter(|r| r.0.len() >= (r.1 * r.2 * 4) as usize).map(|r| Img::de_rgba(r.0, r.1 as usize, r.2 as usize));
    geometria::corregir_rotulos(d, de_la_nube, alto_letra, img.as_ref());
}
