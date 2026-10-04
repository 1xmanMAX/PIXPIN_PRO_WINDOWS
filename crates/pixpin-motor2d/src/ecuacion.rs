//! **La ecuacion tipografiada dentro del dibujo** (`motor/Ecuacion.kt` del
//! movil): raices con su signo y su raya, fracciones con su barra, exponentes
//! en alto, parentesis, valor absoluto y las partes con su llave. Nada de
//! `sqrt(2x)` escrito a maquina: lo que sale es la ecuacion como se escribe
//! en la pizarra.
//!
//! Todo son **textos y lineas del lienzo** —vectoriales— y no una imagen: se
//! mueven con la grafica, se les cambia el color y salen igual en el PDF, en
//! el SVG, en la pagina web y en el movil (que las lee como lo que son). La
//! raya de la fraccion, el signo de la raiz y la llave se dibujan con lineas,
//! que es lo que permite que crezcan con lo que envuelven; una letra no
//! crece.
//!
//! **Sin KaTeX ni MathJax**, ni en el movil ni aqui: el movil compone con sus
//! propias cajas, y esto es el mismo compositor en Rust puro. Un WebView2
//! para escribir una fraccion seria cien megas de memoria por nada.
//!
//! ## Como se compone
//!
//! Cada trozo del arbol de [`crate::formula`] se convierte en una **caja**
//! —ancho, alto y donde esta su eje, que es la linea por la que se alinean el
//! `+` y el `=`— y las cajas se ponen en fila o en columna. Una fraccion es
//! una columna con el eje en la barra; un exponente, una fila con la segunda
//! caja mas pequena y subida; una raiz, su argumento con el signo delante y la
//! raya encima. Aqui cada caja guarda sus piezas ya colocadas respecto de su
//! esquina (el movil guardaba un cierre que las pintaba): componer es correr
//! las piezas de los hijos, y pintar la ecuacion es convertir las piezas de la
//! caja de fuera en elementos.
//!
//! Las letras se miden con lo que pase quien llama (`Medir`): el editor pasa
//! DirectWrite; las pruebas, una cuenta fija.

use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::formula::{Compilada, Nodo};
use crate::vector::Punto2;

/// Lo que ocupa un texto de un tamano: ancho y alto. Es `MedidaDeTexto` del
/// movil.
pub type Medir<'a> = &'a dyn Fn(&str, f32) -> (f32, f32);

/// Cuanto encoge un exponente.
const ENCOGE: f32 = 0.7;
/// Lo menos que se deja encoger una letra, o la tercera planta no se lee.
const MINIMO: f32 = 7.0;

/// Con que se pinta: el color del pincel y la letra.
#[derive(Debug, Clone, PartialEq)]
pub struct Estilo {
    pub color: ColorRgba,
    pub tam: f32,
    pub opacidad: f32,
    pub familia: String,
}

impl Estilo {
    pub fn con_tam(&self, tam: f32) -> Estilo {
        Estilo {
            tam,
            ..self.clone()
        }
    }

    pub fn con_color(&self, color: ColorRgba) -> Estilo {
        Estilo {
            color,
            ..self.clone()
        }
    }
}

/// Una pieza ya colocada respecto de la esquina de su caja.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    Texto {
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        texto: String,
        tam: f32,
        cursiva: bool,
    },
    Raya {
        puntos: Vec<Punto2>,
        grosor: f32,
    },
}

impl Pieza {
    fn corrida(mut self, dx: f32, dy: f32) -> Pieza {
        match &mut self {
            Pieza::Texto { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Pieza::Raya { puntos, .. } => {
                for p in puntos {
                    p.x += dx;
                    p.y += dy;
                }
            }
        }
        self
    }
}

/// Una caja compuesta: lo que mide, donde esta su eje y sus piezas.
#[derive(Debug, Clone, PartialEq)]
pub struct Caja {
    pub ancho: f32,
    pub alto: f32,
    /// A que altura desde arriba pasa el eje: donde va el «=» y la barra de
    /// una fraccion.
    pub eje: f32,
    pub piezas: Vec<Pieza>,
}

impl Caja {
    fn piezas_en(&self, dx: f32, dy: f32) -> impl Iterator<Item = Pieza> + '_ {
        self.piezas.iter().cloned().map(move |p| p.corrida(dx, dy))
    }

    /// La caja pintada con su esquina en `(x, y)`, como elementos del lienzo.
    pub fn elementos(&self, x: f32, y: f32, estilo: &Estilo) -> Vec<Elemento> {
        self.piezas_en(x, y)
            .map(|p| a_elemento(p, estilo))
            .collect()
    }
}

/// Un texto del lienzo con su caja ya medida.
pub fn elemento_texto(
    texto: &str,
    x: f32,
    y: f32,
    (ancho, alto): (f32, f32),
    tam: f32,
    cursiva: bool,
    estilo: &Estilo,
) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: texto.to_string(),
            tam,
            familia: estilo.familia.clone(),
        },
        x,
        y,
        ancho,
        alto,
        trazo: estilo.color,
        opacidad: estilo.opacidad,
        grosor: 1.0,
        rugosidad: 0.0,
        extras: crate::elemento::Extras {
            cursiva,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Una linea del lienzo por esos puntos, recta y sin temblor: una ecuacion,
/// una grafica o una tabla se quieren limpias, no a mano alzada.
pub fn elemento_linea(puntos: Vec<Punto2>, grosor: f32, estilo: &Estilo) -> Elemento {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &puntos {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let (x, y) = puntos.first().map_or((0.0, 0.0), |p| (p.x, p.y));
    Elemento {
        figura: Figura::Linea { puntos },
        x,
        y,
        ancho: (x1 - x0).max(0.0),
        alto: (y1 - y0).max(0.0),
        trazo: estilo.color,
        opacidad: estilo.opacidad,
        grosor,
        rugosidad: 0.0,
        ..Default::default()
    }
}

fn a_elemento(p: Pieza, estilo: &Estilo) -> Elemento {
    match p {
        Pieza::Texto {
            x,
            y,
            ancho,
            alto,
            texto,
            tam,
            cursiva,
        } => elemento_texto(&texto, x, y, (ancho, alto), tam, cursiva, estilo),
        Pieza::Raya { puntos, grosor } => elemento_linea(puntos, grosor, estilo),
    }
}

/// La ecuacion `nombre = formula` entera, para saber cuanto ocupa antes de
/// colocarla. Con varias partes, una llave las abarca y cada una lleva su
/// condicion.
pub fn caja_de_la_ecuacion(nombre: &str, formula: &Compilada, t: f32, medir: Medir<'_>) -> Caja {
    let cabeza = fila(vec![letra(&format!("{nombre} ="), t, false, medir)]);
    if formula.partes.len() == 1 && formula.partes[0].condicion.is_none() {
        return fila(vec![
            cabeza,
            hueco(t * 0.3),
            componer(&formula.partes[0].expresion, t, medir),
        ]);
    }
    // Por partes: una columna de «expresion  si  condicion» y una llave delante.
    let filas: Vec<Caja> = formula
        .partes
        .iter()
        .map(|p| match &p.condicion {
            None => componer(&p.expresion, t, medir),
            Some(c) => fila(vec![
                componer(&p.expresion, t, medir),
                hueco(t * 0.6),
                letra("si", t * 0.85, false, medir),
                hueco(t * 0.4),
                componer(c, t * 0.85, medir),
            ]),
        })
        .collect();
    let col = columna(filas, t * 0.35);
    let alto = col.alto;
    fila(vec![
        cabeza,
        hueco(t * 0.3),
        llave(alto, t),
        hueco(t * 0.2),
        col,
    ])
}

/// La ecuacion como elementos, con su esquina en `(x, y)`.
pub fn elementos(
    nombre: &str,
    formula: &Compilada,
    x: f32,
    y: f32,
    estilo: &Estilo,
    medir: Medir<'_>,
) -> Vec<Elemento> {
    caja_de_la_ecuacion(nombre, formula, estilo.tam, medir).elementos(x, y, estilo)
}

// ---------------------------------------------------------------------
// Del arbol a cajas
// ---------------------------------------------------------------------

fn componer(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    match n {
        Nodo::Numero { texto, .. } => letra(texto, t, false, medir),
        Nodo::Variable(c) => letra(&c.to_string(), t, true, medir),
        Nodo::Constante { nombre, .. } => {
            let texto = if *nombre == "pi" { "π" } else { nombre };
            letra(texto, t, *nombre == "e", medir)
        }
        Nodo::Grupo(a) => entre_parentesis(componer(a, t, medir), t, medir),
        Nodo::Negado(a) => fila(vec![
            letra("−", t, false, medir),
            con_parentesis_si_suma(a, t, medir),
        ]),
        Nodo::Binaria {
            op,
            a,
            b,
            implicita,
        } => binaria(*op, a, b, *implicita, t, medir),
        Nodo::Funcion { nombre, args } => funcion(nombre, args, t, medir),
    }
}

fn binaria(op: char, a: &Nodo, b: &Nodo, implicita: bool, t: f32, medir: Medir<'_>) -> Caja {
    let signo = |s: &str| {
        fila(vec![
            hueco(t * 0.22),
            letra(s, t, false, medir),
            hueco(t * 0.22),
        ])
    };
    let entre = |s: &str| fila(vec![componer(a, t, medir), signo(s), componer(b, t, medir)]);
    match op {
        '+' => entre("+"),
        '-' => fila(vec![
            componer(a, t, medir),
            signo("−"),
            con_parentesis_si_suma(b, t, medir),
        ]),
        '*' if implicita => fila(vec![
            componer(a, t, medir),
            hueco(t * 0.08),
            componer(b, t, medir),
        ]),
        '*' => entre("·"),
        '/' => fraccion(componer(a, t, medir), componer(b, t, medir), t),
        '%' => entre("mod"),
        '^' => potencia(
            con_parentesis_si_compuesto(a, t, medir),
            componer(b, menor(t), medir),
            t,
        ),
        '<' => entre("<"),
        '>' => entre(">"),
        'l' => entre("≤"),
        'g' => entre("≥"),
        '=' => entre("="),
        _ => entre("≠"),
    }
}

fn funcion(nombre: &str, args: &[Nodo], t: f32, medir: Medir<'_>) -> Caja {
    match nombre {
        "sqrt" => raiz(componer(&args[0], t, medir), None, t),
        "cbrt" => raiz(
            componer(&args[0], t, medir),
            Some(letra("3", menor(t), false, medir)),
            t,
        ),
        "abs" => entre_barras(componer(&args[0], t, medir), t),
        "exp" => potencia(
            letra("e", t, true, medir),
            componer(&args[0], menor(t), medir),
            t,
        ),
        "pow" => potencia(
            con_parentesis_si_compuesto(&args[0], t, medir),
            componer(&args[1], menor(t), medir),
            t,
        ),
        "si" => fila(vec![
            componer(&args[1], t, medir),
            hueco(t * 0.5),
            letra("si", t * 0.85, false, medir),
            hueco(t * 0.3),
            componer(&args[0], t * 0.85, medir),
            hueco(t * 0.5),
            letra("si no", t * 0.85, false, medir),
            hueco(t * 0.3),
            componer(&args[2], t, medir),
        ]),
        "log2" => fila(vec![
            subindice(
                letra("log", t, false, medir),
                letra("2", menor(t), false, medir),
                t,
            ),
            entre_parentesis(componer(&args[0], t, medir), t, medir),
        ]),
        _ => {
            let visible = match nombre {
                "asin" => "arcsin",
                "acos" => "arccos",
                "atan" => "arctan",
                n => n,
            };
            let dentro = if args.len() == 1 {
                componer(&args[0], t, medir)
            } else {
                let mut v = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        v.push(letra(",", t, false, medir));
                        v.push(hueco(t * 0.2));
                    }
                    v.push(componer(a, t, medir));
                }
                fila(v)
            };
            fila(vec![
                letra(visible, t, false, medir),
                hueco(t * 0.05),
                entre_parentesis(dentro, t, medir),
            ])
        }
    }
}

fn con_parentesis_si_suma(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    let c = componer(n, t, medir);
    if matches!(n, Nodo::Binaria { op: '+' | '-', .. }) {
        entre_parentesis(c, t, medir)
    } else {
        c
    }
}

fn con_parentesis_si_compuesto(n: &Nodo, t: f32, medir: Medir<'_>) -> Caja {
    let c = componer(n, t, medir);
    if matches!(n, Nodo::Binaria { .. } | Nodo::Negado(_)) {
        entre_parentesis(c, t, medir)
    } else {
        c
    }
}

// ---------------------------------------------------------------------
// Las cajas
// ---------------------------------------------------------------------

fn menor(t: f32) -> f32 {
    (t * ENCOGE).max(MINIMO)
}

/// Un texto suelto. Su eje esta donde una minuscula tiene la mitad de su
/// cuerpo.
fn letra(texto: &str, t: f32, cursiva: bool, medir: Medir<'_>) -> Caja {
    let (ancho, alto) = medir(texto, t);
    Caja {
        ancho,
        alto,
        eje: alto * 0.55,
        piezas: vec![Pieza::Texto {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
            texto: texto.to_string(),
            tam: t,
            cursiva,
        }],
    }
}

fn hueco(ancho: f32) -> Caja {
    Caja {
        ancho,
        alto: 0.0,
        eje: 0.0,
        piezas: Vec::new(),
    }
}

/// Cajas en fila, alineadas por su eje.
fn fila(cajas: Vec<Caja>) -> Caja {
    let arriba = cajas.iter().map(|c| c.eje).fold(0.0f32, f32::max);
    let abajo = cajas.iter().map(|c| c.alto - c.eje).fold(0.0f32, f32::max);
    let mut piezas = Vec::new();
    let mut cursor = 0.0;
    for c in &cajas {
        piezas.extend(c.piezas_en(cursor, arriba - c.eje));
        cursor += c.ancho;
    }
    Caja {
        ancho: cursor,
        alto: arriba + abajo,
        eje: arriba,
        piezas,
    }
}

/// Cajas en columna, alineadas a la izquierda; el eje en medio.
fn columna(cajas: Vec<Caja>, separacion: f32) -> Caja {
    let ancho = cajas.iter().map(|c| c.ancho).fold(0.0f32, f32::max);
    let alto = cajas.iter().map(|c| c.alto).sum::<f32>()
        + separacion * cajas.len().saturating_sub(1) as f32;
    let mut piezas = Vec::new();
    let mut cursor = 0.0;
    for c in &cajas {
        piezas.extend(c.piezas_en(0.0, cursor));
        cursor += c.alto + separacion;
    }
    Caja {
        ancho,
        alto,
        eje: alto / 2.0,
        piezas,
    }
}

/// El grosor de las rayas de la ecuacion: proporcional a la letra.
fn trazo(t: f32) -> f32 {
    (t / 14.0).clamp(0.8, 3.0)
}

fn raya(puntos: &[(f32, f32)], t: f32) -> Pieza {
    Pieza::Raya {
        puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
        grosor: trazo(t),
    }
}

/// Numerador sobre denominador, con la barra en el eje.
fn fraccion(arriba: Caja, abajo: Caja, t: f32) -> Caja {
    let margen = t * 0.15;
    let ancho = arriba.ancho.max(abajo.ancho) + margen * 2.0;
    let hueco = t * 0.18;
    let alto = arriba.alto + hueco * 2.0 + abajo.alto;
    let eje = arriba.alto + hueco;
    let mut piezas: Vec<Pieza> = arriba
        .piezas_en((ancho - arriba.ancho) / 2.0, 0.0)
        .collect();
    piezas.push(raya(&[(0.0, eje), (ancho, eje)], t));
    piezas.extend(abajo.piezas_en((ancho - abajo.ancho) / 2.0, eje + hueco));
    Caja {
        ancho,
        alto,
        eje,
        piezas,
    }
}

/// La base con el exponente pequeno y subido: su pie queda a media altura
/// de la base.
fn potencia(base: Caja, exponente: Caja, t: f32) -> Caja {
    let pie = base.alto * 0.45;
    let sobresale = (exponente.alto - pie).max(0.0);
    let hueco = t * 0.05;
    let mut piezas: Vec<Pieza> = base.piezas_en(0.0, sobresale).collect();
    piezas.extend(exponente.piezas_en(base.ancho + hueco, sobresale + pie - exponente.alto));
    Caja {
        ancho: base.ancho + hueco + exponente.ancho,
        alto: base.alto + sobresale,
        eje: base.eje + sobresale,
        piezas,
    }
}

/// La base con un indice pequeno y bajado.
fn subindice(base: Caja, indice: Caja, t: f32) -> Caja {
    let bajada = indice.alto * 0.5;
    let alto = base
        .alto
        .max(base.alto - base.eje + indice.alto - bajada + base.eje);
    let mut piezas: Vec<Pieza> = base.piezas_en(0.0, 0.0).collect();
    piezas.extend(indice.piezas_en(base.ancho + t * 0.05, base.alto - bajada));
    Caja {
        ancho: base.ancho + indice.ancho + t * 0.05,
        alto,
        eje: base.eje,
        piezas,
    }
}

/// El signo de la raiz dibujado con lineas —crece con lo que envuelve— y la
/// raya encima.
fn raiz(dentro: Caja, indice: Option<Caja>, t: f32) -> Caja {
    let margen = t * 0.15;
    let alto_dentro = dentro.alto + margen;
    let ancho_signo = t * 0.6;
    let ancho_indice = indice.as_ref().map_or(0.0, |i| i.ancho * 0.6);
    let ancho = ancho_indice + ancho_signo + dentro.ancho + margen;
    let alto = alto_dentro + margen;
    let x0 = ancho_indice;
    let mut piezas = vec![raya(
        &[
            (x0, alto * 0.6),
            (x0 + ancho_signo * 0.4, alto),
            (x0 + ancho_signo, 0.0),
            (x0 + ancho_signo + dentro.ancho + margen, 0.0),
        ],
        t,
    )];
    piezas.extend(dentro.piezas_en(x0 + ancho_signo + margen / 2.0, margen * 2.0));
    if let Some(i) = &indice {
        piezas.extend(i.piezas_en(0.0, alto * 0.25 - i.alto / 2.0));
    }
    Caja {
        ancho,
        alto,
        eje: dentro.eje + margen * 2.0,
        piezas,
    }
}

/// Entre parentesis del alto de lo de dentro: de letra si cabe, y si no
/// dibujados.
fn entre_parentesis(dentro: Caja, t: f32, medir: Medir<'_>) -> Caja {
    let abre = letra("(", t, false, medir);
    if dentro.alto <= abre.alto * 1.25 {
        return fila(vec![abre, dentro, letra(")", t, false, medir)]);
    }
    // Altos: dos arcos de tres puntos, que a este tamano se leen igual.
    let ap = t * 0.35;
    let ancho = dentro.ancho + ap * 2.0 + t * 0.2;
    let h = dentro.alto;
    let xd = ancho - ap;
    let mut piezas = vec![raya(&[(ap, 0.0), (ap * 0.3, h * 0.5), (ap, h)], t)];
    piezas.extend(dentro.piezas_en(ap + t * 0.1, 0.0));
    piezas.push(raya(&[(xd, 0.0), (xd + ap * 0.7, h * 0.5), (xd, h)], t));
    Caja {
        ancho,
        alto: dentro.alto,
        eje: dentro.eje,
        piezas,
    }
}

/// Entre barras verticales: el valor absoluto.
fn entre_barras(dentro: Caja, t: f32) -> Caja {
    let margen = t * 0.2;
    let ancho = dentro.ancho + margen * 4.0;
    let h = dentro.alto;
    let mut piezas = vec![raya(&[(margen, 0.0), (margen, h)], t)];
    piezas.extend(dentro.piezas_en(margen * 2.0, 0.0));
    piezas.push(raya(&[(ancho - margen, 0.0), (ancho - margen, h)], t));
    Caja {
        ancho,
        alto: dentro.alto,
        eje: dentro.eje,
        piezas,
    }
}

/// Una llave de la altura de la columna de partes, dibujada con lineas.
fn llave(alto: f32, t: f32) -> Caja {
    let ancho = t * 0.45;
    let pico = t * 0.25;
    Caja {
        ancho,
        alto,
        eje: alto / 2.0,
        piezas: vec![raya(
            &[
                (ancho, 0.0),
                (ancho * 0.5, pico),
                (ancho * 0.5, alto / 2.0 - pico),
                (0.0, alto / 2.0),
                (ancho * 0.5, alto / 2.0 + pico),
                (ancho * 0.5, alto - pico),
                (ancho, alto),
            ],
            t,
        )],
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formula::compilar;

    fn medir(texto: &str, tam: f32) -> (f32, f32) {
        (texto.chars().count() as f32 * tam * 0.6, tam * 1.2)
    }

    fn estilo() -> Estilo {
        Estilo {
            color: ColorRgba::opaco(0.0, 0.0, 0.0),
            tam: 20.0,
            opacidad: 1.0,
            familia: "Segoe UI".into(),
        }
    }

    fn textos(v: &[Elemento]) -> Vec<(String, f32, bool)> {
        v.iter()
            .filter_map(|e| match &e.figura {
                Figura::Texto { texto, tam, .. } => Some((texto.clone(), *tam, e.extras.cursiva)),
                _ => None,
            })
            .collect()
    }

    fn rayas(v: &[Elemento]) -> usize {
        v.iter()
            .filter(|e| matches!(e.figura, Figura::Linea { .. }))
            .count()
    }

    #[test]
    fn una_fraccion_lleva_su_barra_y_el_denominador_debajo() {
        let f = compilar("1/x").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        assert_eq!(rayas(&v), 1, "la barra");
        let uno = v
            .iter()
            .find(|e| {
                textos(std::slice::from_ref(e))
                    .first()
                    .is_some_and(|t| t.0 == "1")
            })
            .unwrap();
        let x = v
            .iter()
            .find(|e| {
                textos(std::slice::from_ref(e))
                    .first()
                    .is_some_and(|t| t.0 == "x")
            })
            .unwrap();
        assert!(x.y > uno.y + 10.0, "la x va debajo del 1");
        // La x de la ecuacion va en cursiva y el numero no.
        assert!(x.extras.cursiva && !uno.extras.cursiva);
    }

    #[test]
    fn el_exponente_sale_pequeno_y_subido_y_no_se_escribe_el_circunflejo() {
        let f = compilar("x^2").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        let t = textos(&v);
        assert!(t.iter().all(|(s, ..)| !s.contains('^')));
        let dos = v
            .iter()
            .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "2"))
            .unwrap();
        let x = v
            .iter()
            .find(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "x"))
            .unwrap();
        assert!(matches!(dos.figura, Figura::Texto { tam, .. } if tam < 20.0));
        assert!(dos.y < x.y, "el 2 va mas alto que la x");
        assert!(dos.x > x.x);
    }

    #[test]
    fn la_raiz_se_dibuja_con_una_raya_que_abarca_lo_de_dentro() {
        let f = compilar("sqrt(x+1)").unwrap();
        let c = caja_de_la_ecuacion("y", &f, 20.0, &medir);
        let v = c.elementos(0.0, 0.0, &estilo());
        assert_eq!(rayas(&v), 1);
        let signo = v
            .iter()
            .find(|e| matches!(e.figura, Figura::Linea { .. }))
            .unwrap();
        let mas_a_la_derecha = v
            .iter()
            .filter(|e| matches!(e.figura, Figura::Texto { .. }))
            .map(|e| e.x + e.ancho)
            .fold(0.0f32, f32::max);
        assert!(signo.x + signo.ancho >= mas_a_la_derecha - 0.5);
        // Caso negativo: nada de «sqrt» a maquina.
        assert!(textos(&v).iter().all(|(s, ..)| s != "sqrt"));
    }

    #[test]
    fn por_partes_lleva_llave_y_cada_parte_su_condicion() {
        let f = compilar("x^2 si x < 0; 2x si x >= 0").unwrap();
        let v = elementos("y", &f, 0.0, 0.0, &estilo(), &medir);
        assert_eq!(textos(&v).iter().filter(|(s, ..)| s == "si").count(), 2);
        assert!(textos(&v).iter().any(|(s, ..)| s == "≥"));
        // La llave: una raya de siete puntos.
        assert!(
            v.iter()
                .any(|e| matches!(&e.figura, Figura::Linea { puntos } if puntos.len() == 7))
        );
    }

    #[test]
    fn las_rayas_de_la_ecuacion_van_sin_temblor() {
        let f = compilar("|x|/2").unwrap();
        let v = elementos("y", &f, 10.0, 10.0, &estilo(), &medir);
        assert!(v.iter().all(|e| e.rugosidad == 0.0));
        assert!(rayas(&v) >= 3);
    }
}
