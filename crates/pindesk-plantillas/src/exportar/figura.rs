//! **A PinDesk**: las paginas como piezas del lienzo (`pindesk_motor2d`),
//! para la ventana de Figuras y para el lienzo de Excalidraw.
//!
//! Se dibujan con las mismas piezas que las plantillas de serie de PinDesk
//! (rectangulos, lineas, elipses y textos, a tiralineas y sin rugosidad),
//! asi que en PinDesk se estampan, se mueven de una vez y se editan pieza a
//! pieza como cualquier dibujo. Las medidas pasan de mm a los pixeles del
//! lienzo (96 por pulgada: un A4 son 794 × 1123, como las de serie).

use pindesk_motor2d::biblioteca::{self, FiguraGuardada};
use pindesk_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pindesk_motor2d::relleno::EstiloRelleno;
use pindesk_motor2d::texto::{self as texto_motor, AlineacionTexto, EstiloDeTexto};
use pindesk_motor2d::vector::Punto2;
use serde_json::{Value, json};

use crate::prim::{PaginaHecha, Prim, Raya};
use crate::tema::{Color, Letra, Tema};
use crate::texto::{self, Medidor};

/// Pixeles del lienzo por milimetro.
pub const PX_POR_MM: f32 = 96.0 / 25.4;
/// Lo que va de una pagina a la siguiente, en fila.
pub const HUECO_PAGINAS: f32 = 40.0;
/// El principio del id de una figura que viene de Plantillas.
pub const PREFIJO_ID: &str = "plantilla-";
/// La categoria con que sale en Figuras («Plantillas · Mi diario»).
pub const CATEGORIA: &str = "Plantillas";

fn c(c: Color) -> ColorRgba {
    let [r, g, b, a] = c.f32s();
    ColorRgba { r, g, b, a }
}

fn base(trazo: ColorRgba, grosor: f32) -> Elemento {
    Elemento {
        trazo,
        grosor,
        rugosidad: 0.0,
        estilo_relleno: EstiloRelleno::Solido,
        ..Default::default()
    }
}

/// Mide con el medidor del lienzo, para que los textos queden en su sitio
/// tambien alli.
struct MedidorLienzo<'a> {
    tema: &'a Tema,
}

impl Medidor for MedidorLienzo<'_> {
    fn ancho(&self, t: &str, tam: f32, letra: Letra, negrita: bool) -> f32 {
        let estilo = EstiloDeTexto {
            negrita,
            ..Default::default()
        };
        texto_motor::medida(t, tam * PX_POR_MM, self.tema.familia(letra), estilo).0 / PX_POR_MM
    }
}

/// **Las piezas de unas paginas**, una al lado de la otra.
pub fn elementos(paginas: &[PaginaHecha], tema: &Tema) -> Vec<Elemento> {
    let mut v = Vec::new();
    let mut dx = 0.0;
    let m = MedidorLienzo { tema };
    let k = PX_POR_MM;
    for p in paginas {
        let punto = |x: f32, y: f32| Punto2::nuevo(dx + x * k, y * k);
        for prim in &p.prims {
            match prim {
                Prim::Rect {
                    caja,
                    relleno,
                    trazo,
                    grosor,
                    radio,
                } => {
                    let trazo_c = trazo.map_or(
                        ColorRgba {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        },
                        c,
                    );
                    let mut e = base(trazo_c, if trazo.is_some() { grosor * k } else { 0.0 });
                    e.figura = Figura::Rectangulo;
                    e.x = dx + caja.x * k;
                    e.y = caja.y * k;
                    e.ancho = caja.w * k;
                    e.alto = caja.h * k;
                    e.relleno = relleno.map(c);
                    e.redondo = *radio > 0.5;
                    v.push(e);
                }
                Prim::Linea {
                    a,
                    b,
                    color,
                    grosor,
                    raya,
                } => {
                    v.push(poli(
                        &[punto(a.0, a.1), punto(b.0, b.1)],
                        c(*color),
                        grosor * k,
                    ));
                    if let Some(e) = v.last_mut() {
                        e.estilo = match raya {
                            Raya::Continua => EstiloTrazo::Solido,
                            Raya::Discontinua => EstiloTrazo::Discontinuo,
                            Raya::Punteada => EstiloTrazo::Punteado,
                        };
                    }
                }
                Prim::Poli {
                    puntos,
                    color,
                    grosor,
                } => {
                    let pts: Vec<Punto2> = puntos.iter().map(|(x, y)| punto(*x, *y)).collect();
                    v.push(poli(&pts, c(*color), grosor * k));
                }
                Prim::Circulo {
                    centro,
                    radio,
                    relleno,
                    trazo,
                    grosor,
                } => v.push(elipse(
                    dx,
                    *centro,
                    *radio,
                    relleno.map(c),
                    trazo.map(c),
                    *grosor,
                )),
                Prim::Puntos {
                    centros,
                    radio,
                    color,
                } => {
                    for q in centros {
                        v.push(elipse(dx, *q, *radio, Some(c(*color)), None, 0.0));
                    }
                }
                Prim::Texto { .. } => {
                    for r in texto::renglones(prim, &m) {
                        let tam = r.tam * k;
                        let familia = tema.familia(r.letra).to_string();
                        let estilo = EstiloDeTexto {
                            negrita: r.negrita,
                            ..Default::default()
                        };
                        let (w, h) = texto_motor::medida(&r.texto, tam, &familia, estilo);
                        let mut e = base(c(r.color), 1.0);
                        e.figura = Figura::Texto {
                            texto: r.texto.clone(),
                            tam,
                            familia,
                        };
                        e.x = dx + r.x * k;
                        // Del renglon a la caja: la linea base menos su ascenso.
                        e.y = (r.base - r.tam * 0.8) * k - (h - tam) / 2.0;
                        e.ancho = w;
                        e.alto = h;
                        e.extras.negrita = r.negrita;
                        e.extras.alineacion = Some(AlineacionTexto::Izquierda);
                        v.push(e);
                    }
                }
            }
        }
        dx += p.ancho * k + HUECO_PAGINAS;
    }
    for (i, e) in v.iter_mut().enumerate() {
        e.id = i as u64 + 1;
    }
    v
}

fn poli(puntos: &[Punto2], color: ColorRgba, grosor: f32) -> Elemento {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for q in puntos {
        x0 = x0.min(q.x);
        y0 = y0.min(q.y);
        x1 = x1.max(q.x);
        y1 = y1.max(q.y);
    }
    let mut e = base(color, grosor);
    e.x = puntos.first().map_or(0.0, |p| p.x);
    e.y = puntos.first().map_or(0.0, |p| p.y);
    e.ancho = (x1 - x0).max(0.0);
    e.alto = (y1 - y0).max(0.0);
    e.figura = Figura::Linea {
        puntos: puntos.to_vec(),
    };
    e
}

fn elipse(
    dx: f32,
    centro: (f32, f32),
    radio: f32,
    relleno: Option<ColorRgba>,
    trazo: Option<ColorRgba>,
    grosor: f32,
) -> Elemento {
    let k = PX_POR_MM;
    let transparente = ColorRgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    let mut e = base(
        trazo.unwrap_or(transparente),
        if trazo.is_some() { grosor * k } else { 0.0 },
    );
    e.figura = Figura::Elipse;
    e.x = dx + (centro.0 - radio) * k;
    e.y = (centro.1 - radio) * k;
    e.ancho = 2.0 * radio * k;
    e.alto = 2.0 * radio * k;
    e.relleno = relleno;
    e
}

/// **La figura para la biblioteca de PinDesk**: `Plantillas · <nombre>`, con
/// id fijo por fichero (volver a mandarla la sustituye).
pub fn figura(clave: &str, nombre: &str, paginas: &[PaginaHecha], tema: &Tema) -> FiguraGuardada {
    let nombre = if nombre.trim().is_empty() {
        clave
    } else {
        nombre.trim()
    };
    FiguraGuardada {
        id: format!("{PREFIJO_ID}{clave}"),
        nombre: format!(
            "{CATEGORIA}{}{nombre}",
            pindesk_motor2d::figuras_de_serie::SEPARADOR
        ),
        elementos: biblioteca::normalizar(&elementos(paginas, tema)),
    }
}

/// **Un `.excalidraw`** con las paginas, como lo abre el lienzo de PinDesk
/// (y excalidraw.com).
pub fn excalidraw(paginas: &[PaginaHecha], tema: &Tema) -> String {
    let piezas = elementos(paginas, tema);
    let doc: Value = json!({
        "type": "excalidraw",
        "version": 2,
        "source": crate::NOMBRE_PRODUCTO,
        "elements": pindesk_motor2d::excalidraw::elementos_a_json(&piezas),
        "appState": {"viewBackgroundColor": "#ffffff", "gridSize": null},
        "files": {},
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}
