//! **Pintar la pagina en la ventana**: las primitivas del nucleo con el
//! `Pintor` de Direct2D, a cualquier aumento (se vuelven a pintar, no se
//! estira un mapa de bits). Los textos se parten una vez, en milimetros,
//! con DirectWrite (`MedidorD2d`) y se guardan con la maqueta: acercar y
//! alejar no los vuelve a medir.

use nucleo::prim::{PaginaHecha, Prim, Raya};
use nucleo::tema::Letra;
use nucleo::texto::{Medidor, Renglon, renglones};
use pindesk_render::letras::{Letra as LetraD2d, SIN_PARTIR};
use pindesk_render::{Color, Pintor, RectF};

/// Lo que se mide a la vez: DirectWrite redondea los pixeles, asi que se
/// mide a 10 px por mm y se divide.
const ESCALA_MEDIR: f32 = 10.0;

/// Un color del nucleo para Direct2D.
pub fn color(c: nucleo::tema::Color) -> Color {
    let [r, g, b, a] = c.f32s();
    Color { r, g, b, a }
}

/// **El medidor de la pantalla**: DirectWrite con las letras de la pagina.
pub struct MedidorD2d<'a> {
    pub familias: &'a [String; 3],
}

impl MedidorD2d<'_> {
    fn familia(&self, l: Letra) -> &str {
        match l {
            Letra::Titulo => &self.familias[0],
            Letra::Cuerpo => &self.familias[1],
            Letra::Mano => &self.familias[2],
        }
    }
}

impl Medidor for MedidorD2d<'_> {
    fn ancho(&self, texto: &str, tam: f32, letra: Letra, negrita: bool) -> f32 {
        let l = LetraD2d {
            familia: self.familia(letra),
            negrita,
            cursiva: false,
            interlineado: None,
        };
        match pindesk_render::letras::medir(texto, tam * ESCALA_MEDIR, &l) {
            Some((w, _)) => w / ESCALA_MEDIR,
            None => nucleo::texto::Estimado.ancho(texto, tam, letra, negrita),
        }
    }

    fn ascenso(&self, _letra: Letra) -> f32 {
        // DirectWrite pinta desde arriba de su renglon: lo que hay de ahi a
        // la linea base en las tres letras de PinDesk.
        0.92
    }
}

/// **Una pagina lista para pintar**: lo maquetado y sus textos partidos.
pub struct Maqueta {
    pub pagina: PaginaHecha,
    /// Los renglones de cada primitiva (vacio si no es texto).
    pub renglones: Vec<Vec<Renglon>>,
}

impl Maqueta {
    pub fn nueva(pagina: PaginaHecha) -> Maqueta {
        let familias = pagina.familias.clone();
        let m = MedidorD2d {
            familias: &familias,
        };
        let renglones = pagina.prims.iter().map(|p| renglones(p, &m)).collect();
        Maqueta { pagina, renglones }
    }
}

/// Donde esta la pagina en la ventana: `px = origen + mm * escala`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vista {
    pub ox: f32,
    pub oy: f32,
    pub escala: f32,
}

impl Vista {
    pub fn x(&self, mm: f32) -> f32 {
        self.ox + mm * self.escala
    }
    pub fn y(&self, mm: f32) -> f32 {
        self.oy + mm * self.escala
    }
    pub fn p(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (self.x(x), self.y(y))
    }
    pub fn caja(&self, c: nucleo::prim::Caja) -> RectF {
        RectF {
            x: self.x(c.x),
            y: self.y(c.y),
            ancho: c.w * self.escala,
            alto: c.h * self.escala,
        }
    }
    /// De la ventana a mm de la pagina.
    pub fn a_mm(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.ox) / self.escala, (y - self.oy) / self.escala)
    }
}

/// Los puntos de un rectangulo de esquinas redondas (para trazarlo).
pub fn contorno_redondo(r: RectF, radio: f32) -> Vec<(f32, f32)> {
    let radio = radio.min(r.ancho / 2.0).min(r.alto / 2.0).max(0.0);
    let mut v = Vec::with_capacity(4 * 7 + 1);
    let esquinas = [
        (r.x + r.ancho - radio, r.y + radio, -90.0f32),
        (r.x + r.ancho - radio, r.y + r.alto - radio, 0.0),
        (r.x + radio, r.y + r.alto - radio, 90.0),
        (r.x + radio, r.y + radio, 180.0),
    ];
    for (cx, cy, desde) in esquinas {
        for k in 0..=6 {
            let a = (desde + 90.0 * k as f32 / 6.0).to_radians();
            v.push((cx + radio * a.cos(), cy + radio * a.sin()));
        }
    }
    if let Some(p) = v.first().copied() {
        v.push(p);
    }
    v
}

/// **Pinta una pagina** con su sombra.
pub fn pagina(p: &Pintor, m: &Maqueta, v: Vista, sombra: bool) {
    let s = v.escala;
    let hoja = RectF {
        x: v.x(0.0),
        y: v.y(0.0),
        ancho: m.pagina.ancho * s,
        alto: m.pagina.alto * s,
    };
    if sombra {
        for (i, a) in [(6.0, 0.05f32), (3.0, 0.07), (1.0, 0.10)] {
            p.rellenar_redondeado(
                RectF {
                    x: hoja.x - i * 0.4,
                    y: hoja.y + i * 0.6,
                    ancho: hoja.ancho + i * 0.8,
                    alto: hoja.alto + i * 0.6,
                },
                i,
                Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a,
                },
            );
        }
    }
    p.con_recorte(hoja, |p| {
        for (prim, rs) in m.pagina.prims.iter().zip(&m.renglones) {
            una(p, prim, rs, v, &m.pagina.familias);
        }
    });
}

fn una(p: &Pintor, prim: &Prim, rs: &[Renglon], v: Vista, familias: &[String; 3]) {
    let s = v.escala;
    let g = |mm: f32| (mm * s).max(0.8);
    match prim {
        Prim::Rect {
            caja,
            relleno,
            trazo,
            grosor,
            radio,
        } => {
            let r = v.caja(*caja);
            let radio = radio * s;
            if let Some(c) = relleno {
                if radio > 0.5 {
                    p.rellenar_redondeado(r, radio, color(*c));
                } else {
                    p.rellenar(r, color(*c));
                }
            }
            if let Some(c) = trazo {
                if radio > 0.5 {
                    p.polilinea(&contorno_redondo(r, radio), g(*grosor), color(*c));
                } else {
                    p.trazar(r, g(*grosor), color(*c));
                }
            }
        }
        Prim::Linea {
            a,
            b,
            color: c,
            grosor,
            raya,
        } => match raya {
            Raya::Continua => p.linea(v.p(*a), v.p(*b), g(*grosor), color(*c)),
            _ => p.polilinea_discontinua(&[v.p(*a), v.p(*b)], g(*grosor), color(*c)),
        },
        Prim::Circulo {
            centro,
            radio,
            relleno,
            trazo,
            grosor,
        } => {
            let cen = v.p(*centro);
            let r = (radio * s).max(0.6);
            if let Some(c) = relleno {
                p.circulo(cen, r, color(*c));
            }
            if let Some(c) = trazo {
                p.anillo(cen, r, g(*grosor), color(*c));
            }
        }
        Prim::Poli {
            puntos,
            color: c,
            grosor,
        } => {
            let pts: Vec<(f32, f32)> = puntos.iter().map(|q| v.p(*q)).collect();
            p.polilinea(&pts, g(*grosor), color(*c));
        }
        Prim::Puntos {
            centros,
            radio,
            color: c,
        } => {
            let r = (radio * s).max(0.7);
            for q in centros {
                p.circulo(v.p(*q), r, color(*c));
            }
        }
        Prim::Texto { .. } => {
            for r in rs {
                let tam = r.tam * s;
                if tam < 1.5 {
                    continue;
                }
                let familia = match r.letra {
                    Letra::Titulo => &familias[0],
                    Letra::Cuerpo => &familias[1],
                    Letra::Mano => &familias[2],
                };
                p.texto_con_letra(
                    &r.texto,
                    v.x(r.x),
                    v.y(r.base) - tam * 0.92,
                    tam,
                    SIN_PARTIR,
                    &LetraD2d {
                        familia,
                        negrita: r.negrita,
                        cursiva: false,
                        interlineado: None,
                    },
                    color(r.color),
                );
            }
        }
    }
}
