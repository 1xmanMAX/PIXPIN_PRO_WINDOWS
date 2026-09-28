//! **El cronograma: un plan dibujado, no una hoja de calculo** (F12;
//! `motor/Cronograma.kt` y `Renderer.drawCronograma` del movil, tipo
//! `pixpin-gantt`).
//!
//! En una reunion el cronograma se dibuja siempre igual: una columna de
//! nombres a la izquierda, una escala arriba y una barra por fila. Hacerlo con
//! rectangulos sueltos son doce elementos que hay que alinear a mano y que se
//! descuadran en cuanto se mueve uno. Aqui es **una figura con datos dentro**:
//! la rejilla se reparte sola dentro de la caja, las barras se arrastran por
//! encima —moverlas y estirarlas es el gesto entero— y anadir una tarea
//! recoloca lo demas. Estirar la figura estira el plan sin descuadrarlo,
//! porque nada esta en coordenadas: todo esta en filas y columnas.
//!
//! Lo que **no** es: un gestor de proyectos. No hay dependencias, ni
//! recursos, ni fechas reales. Es el croquis de cuando va cada cosa.

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Que parte del ancho se lleva la columna de los nombres.
pub const ANCHO_DE_LOS_NOMBRES: f32 = 0.32;
/// Lo que ocupa la fila de la escala, arriba, en tanto por uno del alto.
pub const ALTO_DE_LA_ESCALA: f32 = 0.16;
/// Lo mas fina que se deja una barra, en columnas: por debajo no se agarra.
pub const MINIMA_BARRA: f32 = 0.25;
/// A cuanto engancha el arrastre de una barra: a cuartos de columna.
pub const PASO_DEL_CRONOGRAMA: f32 = 0.25;
/// Cuanto de una barra, por su punta, estira en vez de mover.
const PUNTA_DE_LA_BARRA: f32 = 0.3;
/// Y como mucho esto en pixeles: en una barra larga la punta no es medio metro.
const PUNTA_MAXIMA: f32 = 24.0;
/// El respiro de la barra arriba y abajo dentro de su fila.
const RESPIRO_DE_LA_BARRA: f32 = 0.18;
/// Hasta cuantas columnas: pasadas cuarenta, lo que se ve es una trama.
pub const MAXIMO_DE_PERIODOS: u32 = 40;
/// Con cuantas filas nace: las justas para que se entienda de que va.
pub const FILAS_DE_FABRICA: usize = 3;
/// Y cuantas columnas, si el fichero no lo dice (`periodos = 6` del movil).
pub const PERIODOS_DE_FABRICA: u32 = 6;

/// Una fila: **que es y desde cuando hasta cuando**, en columnas (pueden ser
/// fraccionarias: media columna es media semana).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tarea {
    #[serde(default)]
    pub nombre: String,
    /// Desde que columna arranca, contando desde cero.
    #[serde(default)]
    pub desde: f32,
    /// Cuantas columnas dura.
    #[serde(default = "uno")]
    pub cuanto: f32,
    /// El color de esta barra si tiene el suyo; `None` es el de la figura.
    #[serde(default)]
    pub color: Option<ColorRgba>,
}

fn uno() -> f32 {
    1.0
}

impl Tarea {
    pub fn nueva(desde: f32, cuanto: f32) -> Tarea {
        Tarea {
            nombre: String::new(),
            desde,
            cuanto,
            color: None,
        }
    }
}

/// **Las filas con que nace**: tres, escalonadas, cada una detras de la
/// anterior —porque es lo que es un plan—. Una figura en blanco obliga a
/// descubrir donde se le anaden cosas antes de que ensene nada.
pub fn tareas_de_fabrica() -> Vec<Tarea> {
    (0..FILAS_DE_FABRICA).map(|i| Tarea::nueva(i as f32, 1.0)).collect()
}

fn datos(e: &Elemento) -> Option<(&[Tarea], u32)> {
    match &e.figura {
        Figura::Cronograma { tareas, periodos } => Some((tareas, *periodos)),
        _ => None,
    }
}

/// La caja del elemento, ordenada.
fn caja(e: &Elemento) -> (f32, f32, f32, f32) {
    let (x0, x1) = (e.x.min(e.x + e.ancho), e.x.max(e.x + e.ancho));
    let (y0, y1) = (e.y.min(e.y + e.alto), e.y.max(e.y + e.alto));
    (x0, y0, x1, y1)
}

/// Donde empieza la zona de las barras: a la derecha de los nombres.
pub fn x_de_la_escala(e: &Elemento) -> f32 {
    let (x0, _, x1, _) = caja(e);
    x0 + (x1 - x0) * ANCHO_DE_LOS_NOMBRES
}

/// Donde empieza la primera fila: debajo de la escala.
pub fn y_de_las_filas(e: &Elemento) -> f32 {
    let (_, y0, _, y1) = caja(e);
    y0 + (y1 - y0) * ALTO_DE_LA_ESCALA
}

/// Lo que mide una columna de la escala.
pub fn ancho_de_columna(e: &Elemento) -> f32 {
    let Some((_, periodos)) = datos(e) else {
        return 0.0;
    };
    let (_, _, x1, _) = caja(e);
    (x1 - x_de_la_escala(e)) / periodos.max(1) as f32
}

/// Lo que mide una fila. Con cero tareas, la caja entera.
pub fn alto_de_fila(e: &Elemento) -> f32 {
    let Some((tareas, _)) = datos(e) else {
        return 0.0;
    };
    let (_, _, _, y1) = caja(e);
    (y1 - y_de_las_filas(e)) / tareas.len().max(1) as f32
}

/// La caja de la barra de la tarea `i`, en coordenadas del dibujo. No ocupa
/// la fila entera de alto: deja un respiro para que dos filas seguidas no se
/// lean como un bloque.
pub fn barra_de_tarea(e: &Elemento, i: usize) -> Option<(f32, f32, f32, f32)> {
    let (tareas, _) = datos(e)?;
    let t = tareas.get(i)?;
    let col = ancho_de_columna(e);
    let fila = alto_de_fila(e);
    if col <= 0.0 || fila <= 0.0 {
        return None;
    }
    let x0 = x_de_la_escala(e) + t.desde * col;
    let y0 = y_de_las_filas(e) + i as f32 * fila;
    let respiro = fila * RESPIRO_DE_LA_BARRA;
    Some((x0, y0 + respiro, x0 + t.cuanto.max(MINIMA_BARRA) * col, y0 + fila - respiro))
}

/// Las rayas verticales de la escala, de la primera a la ultima.
pub fn columnas(e: &Elemento) -> Vec<f32> {
    let Some((_, periodos)) = datos(e) else {
        return Vec::new();
    };
    let col = ancho_de_columna(e);
    if col <= 0.0 {
        return Vec::new();
    }
    (0..=periodos.max(1)).map(|k| x_de_la_escala(e) + k as f32 * col).collect()
}

/// **De que tamano va la letra, sin poder reventar.** En el movil una cuenta
/// con `coerceIn(1.0, alto * 0.8)` lanzaba con filas de menos de pixel y
/// medio, y al dibujar: el tope no baja nunca de uno.
pub fn letra(alto_de_fila: f32) -> f32 {
    let pedida = alto_de_fila * 0.5;
    let tope = (alto_de_fila * 0.8).max(1.0);
    pedida.clamp(1.0, tope)
}

/// **La letra de un cronograma en su fila** (`letraDelCronograma` del movil):
/// la suya (`fontSize`, la del pincel al crearlo) si la tiene, y si no la
/// mitad del alto de la fila; nunca mas del 80 % de la fila ni menos de uno.
pub fn letra_de(e: &Elemento, alto_de_fila: f32) -> f32 {
    let tope = (alto_de_fila * 0.8).max(1.0);
    match e.extras.tam_letra {
        Some(t) if t.is_finite() && t > 0.0 => t.clamp(1.0, tope),
        _ => letra(alto_de_fila),
    }
}

/// Con que letra se rotula: la suya (`fontFamily`), o la de fabrica del
/// lienzo, que es la de los textos que se escriben alrededor.
pub fn familia_de(e: &Elemento) -> &str {
    e.extras
        .familia
        .as_deref()
        .unwrap_or_else(|| crate::texto::nombre_de_familia(None))
}

/// Lo que mide de ancho un renglon con la letra del cronograma: el medidor
/// de la aplicacion si esta puesto (DirectWrite), la cuenta a ojo si no.
fn ancho_con(texto: &str, tam: f32, familia: &str) -> f32 {
    crate::texto::medida(texto, tam, familia, Default::default()).0
}

/// **Lo ancha que tiene que ser la figura para que quepa cada nombre**, o
/// `None` si ya caben. El cajetin la ensancha con esto al poner nombres:
/// recortarlos («Cimien…») es lo que hacia el movil y es lo que se veia feo.
pub fn ancho_para_los_nombres(e: &Elemento) -> Option<f32> {
    let (tareas, _) = datos(e)?;
    let tam = letra_de(e, alto_de_fila(e));
    let familia = familia_de(e);
    let mas_largo = tareas
        .iter()
        .map(|t| ancho_con(&t.nombre, tam, familia))
        .fold(0.0f32, f32::max);
    // El hueco de los nombres es `ANCHO_DE_LOS_NOMBRES` del ancho menos el
    // aire de los dos lados (`tam * 0.6`, el de `ordenes`).
    let hace_falta = (mas_largo + tam * 0.6) / ANCHO_DE_LOS_NOMBRES;
    (hace_falta > e.ancho.abs() + 0.5).then_some(hace_falta.ceil())
}

/// Que se hace al agarrar una barra: moverla entera o estirarla por la punta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManoEnLaBarra {
    Mover,
    Estirar,
}

/// Que barra toca el cursor, y si por la punta. Por el cuerpo se mueve y por
/// la punta derecha se estira: no hace falta un tirador por barra.
pub fn toque_en_barra(e: &Elemento, p: Punto2, margen: f32) -> Option<(usize, ManoEnLaBarra)> {
    let (tareas, _) = datos(e)?;
    for i in 0..tareas.len() {
        let Some((x0, y0, x1, y1)) = barra_de_tarea(e, i) else {
            continue;
        };
        if p.y < y0 - margen || p.y > y1 + margen || p.x < x0 - margen || p.x > x1 + margen {
            continue;
        }
        let punta = ((x1 - x0) * PUNTA_DE_LA_BARRA).min(PUNTA_MAXIMA);
        let mano = if p.x >= x1 - punta {
            ManoEnLaBarra::Estirar
        } else {
            ManoEnLaBarra::Mover
        };
        return Some((i, mano));
    }
    None
}

fn enganchado(v: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    (v / PASO_DEL_CRONOGRAMA).round() * PASO_DEL_CRONOGRAMA
}

/// **La tarea `i` despues de arrastrarla hasta `p`**, enganchada a cuartos de
/// columna: a pulso salen barras que empiezan en 2,37 y el dibujo deja de
/// decir «esta empieza cuando acaba aquella». `agarre` es por donde se cogio
/// la barra, en columnas desde su principio (sin el, la barra saltaria).
pub fn tarea_arrastrada(e: &Elemento, i: usize, mano: ManoEnLaBarra, p: Punto2, agarre: f32) -> Option<Tarea> {
    let (tareas, periodos) = datos(e)?;
    let t = tareas.get(i)?;
    let col = ancho_de_columna(e);
    if col <= 0.0 {
        return None;
    }
    let en_columnas = (p.x - x_de_la_escala(e)) / col;
    let periodos = periodos as f32;
    let mut nueva = t.clone();
    match mano {
        ManoEnLaBarra::Mover => {
            nueva.desde = enganchado(en_columnas - agarre).clamp(0.0, (periodos - t.cuanto).max(0.0));
        }
        ManoEnLaBarra::Estirar => {
            nueva.cuanto = enganchado(en_columnas - t.desde)
                .clamp(MINIMA_BARRA, (periodos - t.desde).max(MINIMA_BARRA));
        }
    }
    Some(nueva)
}

/// **Una tarea mas, detras de la ultima y del mismo largo**: nueve de cada
/// diez veces es lo que se quiere, y corregirlo es arrastrar. Si no cabe, la
/// escala crece. `false` si `e` no es un cronograma.
pub fn con_tarea_nueva(e: &mut Elemento, nombre: &str) -> bool {
    let Figura::Cronograma { tareas, periodos } = &mut e.figura else {
        return false;
    };
    let ultima = tareas.last();
    let desde = ultima.map_or(0.0, |t| t.desde + t.cuanto);
    let cuanto = ultima.map_or(1.0, |t| t.cuanto);
    *periodos = (*periodos).max((desde + cuanto).ceil() as u32).min(MAXIMO_DE_PERIODOS);
    let tope = (*periodos as f32 - cuanto).max(0.0);
    tareas.push(Tarea {
        nombre: nombre.to_string(),
        desde: desde.min(tope),
        cuanto,
        color: None,
    });
    e.tocar();
    true
}

/// Una tarea menos: la ultima. Sin ninguna, se queda como esta.
pub fn sin_la_ultima_tarea(e: &mut Elemento) -> bool {
    let Figura::Cronograma { tareas, .. } = &mut e.figura else {
        return false;
    };
    if tareas.pop().is_none() {
        return false;
    }
    e.tocar();
    true
}

/// Una columna mas o menos en la escala, sin bajar de una ni pasar del tope.
pub fn con_periodos(e: &mut Elemento, cuantos: i64) -> bool {
    let Figura::Cronograma { periodos, .. } = &mut e.figura else {
        return false;
    };
    let nuevo = cuantos.clamp(1, MAXIMO_DE_PERIODOS as i64) as u32;
    if nuevo == *periodos {
        return false;
    }
    *periodos = nuevo;
    e.tocar();
    true
}

/// El nombre recortado a lo que cabe en `ancho` (a ojo: el motor no mide
/// letras), con puntos suspensivos si no cabe entero.
fn recortado(nombre: &str, ancho: f32, tam: f32, familia: &str) -> String {
    let cabe = |s: &str| ancho_con(s, tam, familia) <= ancho;
    if cabe(nombre) {
        return nombre.to_string();
    }
    let mut s: String = nombre.to_string();
    while !s.is_empty() && !cabe(&format!("{s}…")) {
        s.pop();
    }
    if s.is_empty() { String::new() } else { format!("{s}…") }
}

/// Un rectangulo de esquinas redondas como poligono.
fn redondeado(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Vec<Punto2> {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut v = Vec::with_capacity(4 * 7);
    let esquinas = [
        (x1 - r, y0 + r, -std::f32::consts::FRAC_PI_2),
        (x1 - r, y1 - r, 0.0),
        (x0 + r, y1 - r, std::f32::consts::FRAC_PI_2),
        (x0 + r, y0 + r, std::f32::consts::PI),
    ];
    for (cx, cy, a0) in esquinas {
        for k in 0..=6 {
            let a = a0 + std::f32::consts::FRAC_PI_2 * k as f32 / 6.0;
            v.push(Punto2::nuevo(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    v
}

/// **Lo que se pinta de un cronograma** (`drawCronograma`): la rejilla fina,
/// el marco, los numeros de la escala, las barras y los nombres. Liso, sin el
/// temblor del resto: un plan es un instrumento de lectura, y una rejilla
/// temblorosa hace que dos columnas no parezcan de la misma anchura.
pub fn ordenes(e: &Elemento, tinta: ColorRgba) -> Vec<Orden> {
    let Some((tareas, periodos)) = datos(e) else {
        return Vec::new();
    };
    let (x0, y0, x1, y1) = caja(e);
    if x1 - x0 <= 1.0 || y1 - y0 <= 1.0 {
        return Vec::new();
    }
    let relleno = e
        .relleno
        .filter(|c| c.a > 0.0)
        .map(|c| ColorRgba {
            a: c.a * e.opacidad.clamp(0.0, 1.0),
            ..c
        })
        .unwrap_or(tinta);
    let grosor = e.grosor.max(1.0);
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let girar = |p: Punto2| {
        if e.angulo == 0.0 {
            p
        } else {
            p.girar(centro, e.angulo)
        }
    };
    let raya = |a: (f32, f32), b: (f32, f32), color: ColorRgba, g: f32| Orden::Polilinea {
        puntos: vec![girar(Punto2::nuevo(a.0, a.1)), girar(Punto2::nuevo(b.0, b.1))],
        color,
        grosor: g,
        estilo: EstiloTrazo::Solido,
    };
    let mut v = Vec::new();
    // La rejilla: las columnas de la escala, finas, y las filas.
    let fina = ColorRgba {
        a: tinta.a * 0.45,
        ..tinta
    };
    for x in columnas(e) {
        v.push(raya((x, y0), (x, y1), fina, grosor));
    }
    let filas = y_de_las_filas(e);
    let alto = alto_de_fila(e);
    for i in 0..=tareas.len() {
        let y = filas + i as f32 * alto;
        v.push(raya((x0, y), (x1, y), fina, grosor));
    }
    // El marco, con el trazo entero.
    v.push(Orden::Polilinea {
        puntos: [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
            .iter()
            .map(|&(x, y)| girar(Punto2::nuevo(x, y)))
            .collect(),
        color: tinta,
        grosor,
        estilo: EstiloTrazo::Solido,
    });
    // Con la letra del cronograma (la del pincel) y medida con el medidor de
    // la aplicacion: antes salia en la del sistema y medida a ojo, distinta
    // de los textos de al lado.
    let familia = familia_de(e);
    let tam = letra_de(e, alto);
    let alto_letra = crate::texto::medida("Ag", tam, familia, Default::default()).1;
    // Los numeros de la escala, uno por columna y centrados en ella.
    let col = ancho_de_columna(e);
    let tam_escala = letra_de(e, (filas - y0).max(1.0));
    let alto_escala = crate::texto::medida("0", tam_escala, familia, Default::default()).1;
    let escala_y = y0 + (filas - y0 - alto_escala).max(0.0) / 2.0;
    for k in 0..periodos.max(1) {
        let texto = (k + 1).to_string();
        let w = ancho_con(&texto, tam_escala, familia);
        let cx = x_de_la_escala(e) + (k as f32 + 0.5) * col;
        v.push(Orden::Texto {
            texto,
            x: cx - w / 2.0,
            y: escala_y,
            tam: tam_escala,
            familia: familia.to_string(),
            color: tinta,
            ancho_max: w + tam_escala,
            negrita: false,
            cursiva: false,
        });
    }
    // Y cada fila: su barra sobre la rejilla y su nombre a la izquierda. Sin
    // nombre se rotula el numero de fila: una figura recien nacida tiene que
    // decir algo.
    for (i, t) in tareas.iter().enumerate() {
        let Some((bx0, by0, bx1, by1)) = barra_de_tarea(e, i) else {
            continue;
        };
        v.push(Orden::Relleno {
            puntos: redondeado(bx0, by0, bx1, by1, (by1 - by0) / 3.0)
                .into_iter()
                .map(girar)
                .collect(),
            color: t.color.unwrap_or(relleno),
        });
        let rotulo = if t.nombre.trim().is_empty() {
            (i + 1).to_string()
        } else {
            t.nombre.clone()
        };
        let hueco = x_de_la_escala(e) - x0 - tam * 0.6;
        let texto = recortado(&rotulo, hueco, tam, familia);
        if !texto.is_empty() {
            v.push(Orden::Texto {
                texto,
                x: x0 + tam * 0.3,
                y: (by0 + by1) / 2.0 - alto_letra / 2.0,
                tam,
                familia: familia.to_string(),
                color: tinta,
                ancho_max: hueco.max(1.0) + tam,
                negrita: false,
                cursiva: false,
            });
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn plan(x: f32, y: f32, ancho: f32, alto: f32, tareas: Vec<Tarea>, periodos: u32) -> Elemento {
        Elemento {
            figura: Figura::Cronograma { tareas, periodos },
            x,
            y,
            ancho,
            alto,
            ..Default::default()
        }
    }

    fn ab() -> Vec<Tarea> {
        vec![
            Tarea {
                nombre: "A".into(),
                desde: 0.0,
                cuanto: 2.0,
                color: None,
            },
            Tarea {
                nombre: "B".into(),
                desde: 2.0,
                cuanto: 1.0,
                color: None,
            },
        ]
    }

    #[test]
    fn nace_con_filas_escalonadas_para_que_ensene_algo() {
        let t = tareas_de_fabrica();
        assert_eq!(t.len(), 3);
        assert_eq!((t[0].desde, t[1].desde, t[2].desde), (0.0, 1.0, 2.0));
    }

    #[test]
    fn la_rejilla_se_reparte_sola() {
        let e = plan(0.0, 0.0, 400.0, 200.0, ab(), 8);
        let c = columnas(&e);
        assert_eq!(c.len(), 9);
        assert!((c[0] - x_de_la_escala(&e)).abs() < 1e-4);
        assert!((c[8] - 400.0).abs() < 1e-3);
        let anchos: Vec<f32> = c.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(anchos.iter().all(|a| (a - anchos[0]).abs() < 1e-3));
    }

    #[test]
    fn estirar_la_figura_no_descuadra_el_plan() {
        let chico = plan(0.0, 0.0, 400.0, 200.0, ab(), 8);
        let grande = plan(0.0, 0.0, 800.0, 300.0, ab(), 8);
        let en_columnas = |e: &Elemento| {
            let (x0, _, x1, _) = barra_de_tarea(e, 1).unwrap();
            ((x0 - x_de_la_escala(e)) / ancho_de_columna(e), (x1 - x0) / ancho_de_columna(e))
        };
        let (a, b) = (en_columnas(&chico), en_columnas(&grande));
        assert!((a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4);
    }

    #[test]
    fn cada_barra_se_queda_en_su_fila() {
        let e = plan(10.0, 20.0, 400.0, 200.0, ab(), 8);
        for i in 0..2 {
            let (_, y0, _, y1) = barra_de_tarea(&e, i).unwrap();
            let techo = y_de_las_filas(&e) + i as f32 * alto_de_fila(&e);
            assert!(y0 >= techo && y1 <= techo + alto_de_fila(&e) + 1e-3);
        }
        assert!(barra_de_tarea(&e, 2).is_none(), "no hay tercera");
    }

    #[test]
    fn por_el_cuerpo_se_mueve_y_por_la_punta_se_estira_a_cuartos_de_columna() {
        let e = plan(0.0, 0.0, 400.0, 200.0, ab(), 8);
        let (x0, y0, x1, y1) = barra_de_tarea(&e, 0).unwrap();
        let medio = Punto2::nuevo((x0 + x1) / 2.0 - 20.0, (y0 + y1) / 2.0);
        assert_eq!(toque_en_barra(&e, medio, 0.0), Some((0, ManoEnLaBarra::Mover)));
        let punta = Punto2::nuevo(x1 - 2.0, (y0 + y1) / 2.0);
        assert_eq!(toque_en_barra(&e, punta, 0.0), Some((0, ManoEnLaBarra::Estirar)));
        // Caso negativo: en la columna de los nombres no hay barra.
        assert_eq!(toque_en_barra(&e, Punto2::nuevo(5.0, (y0 + y1) / 2.0), 0.0), None);
        // Mover: agarrada por su principio, llevada a la columna 1,3 -> 1,25.
        let col = ancho_de_columna(&e);
        let p = Punto2::nuevo(x_de_la_escala(&e) + 1.3 * col, 0.0);
        let t = tarea_arrastrada(&e, 0, ManoEnLaBarra::Mover, p, 0.0).unwrap();
        assert_eq!(t.desde, 1.25);
        // Sin salirse de la escala: hasta 8 - 2.
        let lejos = Punto2::nuevo(x_de_la_escala(&e) + 20.0 * col, 0.0);
        assert_eq!(tarea_arrastrada(&e, 0, ManoEnLaBarra::Mover, lejos, 0.0).unwrap().desde, 6.0);
        // Estirar: nunca menos de un cuarto.
        let atras = Punto2::nuevo(x_de_la_escala(&e) - 50.0, 0.0);
        assert_eq!(tarea_arrastrada(&e, 0, ManoEnLaBarra::Estirar, atras, 0.0).unwrap().cuanto, MINIMA_BARRA);
    }

    #[test]
    fn una_tarea_nueva_va_detras_de_la_ultima_y_la_escala_crece_si_no_cabe() {
        let mut e = plan(0.0, 0.0, 400.0, 200.0, ab(), 3);
        assert!(con_tarea_nueva(&mut e, "C"));
        let Figura::Cronograma { tareas, periodos } = &e.figura else {
            unreachable!()
        };
        assert_eq!(tareas[2].desde, 3.0);
        assert_eq!(*periodos, 4);
        assert!(sin_la_ultima_tarea(&mut e));
        assert!(con_periodos(&mut e, 100));
        assert!(matches!(e.figura, Figura::Cronograma { periodos: MAXIMO_DE_PERIODOS, .. }));
        // Caso negativo: a una caja normal no se le anaden tareas.
        let mut r = Elemento::default();
        assert!(!con_tarea_nueva(&mut r, "x"));
    }

    #[test]
    fn la_letra_no_revienta_con_filas_diminutas() {
        assert_eq!(letra(0.5), 1.0);
        assert_eq!(letra(40.0), 20.0);
    }

    #[test]
    fn se_pinta_rejilla_marco_numeros_barras_y_nombres() {
        let e = plan(0.0, 0.0, 400.0, 200.0, ab(), 4);
        let o = ordenes(&e, ColorRgba::opaco(0.0, 0.0, 0.0));
        let textos: Vec<&str> = o
            .iter()
            .filter_map(|x| match x {
                Orden::Texto { texto, .. } => Some(texto.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(textos, vec!["1", "2", "3", "4", "A", "B"]);
        assert_eq!(o.iter().filter(|x| matches!(x, Orden::Relleno { .. })).count(), 2);
        // Cinco columnas de escala (4 + 1), tres rayas de fila (2 + 1) y el marco.
        assert_eq!(o.iter().filter(|x| matches!(x, Orden::Polilinea { .. })).count(), 5 + 3 + 1);
    }

    #[test]
    fn un_nombre_largo_se_recorta_con_puntos_suspensivos() {
        let s = recortado("Una tarea con un nombre larguisimo", 60.0, 10.0, "Excalifont");
        assert!(s.ends_with('…') && s.chars().count() < 20);
        assert_eq!(recortado("A", 60.0, 10.0, "Excalifont"), "A");
    }
}

#[cfg(test)]
mod pruebas_de_la_letra {
    use super::*;

    fn con_nombres(nombres: &[&str], ancho: f32, tam: Option<f32>) -> Elemento {
        let tareas = nombres
            .iter()
            .enumerate()
            .map(|(i, n)| Tarea { nombre: n.to_string(), ..Tarea::nueva(i as f32, 1.0) })
            .collect();
        let mut e = Elemento {
            figura: Figura::Cronograma { tareas, periodos: 6 },
            ancho,
            alto: 200.0,
            ..Default::default()
        };
        e.extras.tam_letra = tam;
        e.extras.familia = Some("Excalifont".into());
        e
    }

    fn textos(e: &Elemento) -> Vec<(String, f32, String)> {
        ordenes(e, ColorRgba::opaco(0.0, 0.0, 0.0))
            .into_iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, tam, familia, .. } => Some((texto, tam, familia)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn los_nombres_van_con_la_letra_y_el_tamano_del_pincel() {
        // Como `letraDelCronograma` del movil: con `fontSize` puesto manda el.
        let e = con_nombres(&["Cimientos", "Muros", "Techo"], 600.0, Some(20.0));
        let t = textos(&e);
        assert!(t.iter().any(|(s, _, _)| s == "Cimientos"), "{t:?}");
        assert!(t.iter().all(|(_, _, f)| f == "Excalifont"), "{t:?}");
        assert!(t.iter().filter(|(s, _, _)| s.len() > 2).all(|(_, tam, _)| *tam == 20.0));
    }

    #[test]
    fn sin_su_tamano_la_letra_sale_de_la_fila_como_antes() {
        // Caso negativo: un cronograma viejo o del movil sin `fontSize`.
        let e = con_nombres(&["A"], 400.0, None);
        let fila = alto_de_fila(&e);
        assert_eq!(letra_de(&e, fila), letra(fila));
        // Y un tamano pedido mayor que la fila no la revienta.
        let e = con_nombres(&["A"], 400.0, Some(500.0));
        assert!(letra_de(&e, alto_de_fila(&e)) <= alto_de_fila(&e) * 0.8);
    }

    #[test]
    fn la_figura_pide_el_ancho_justo_para_que_quepa_el_nombre_mas_largo() {
        let e = con_nombres(&["Estructura y muros de carga", "B"], 200.0, Some(20.0));
        let ancho = ancho_para_los_nombres(&e).expect("no cabe y no pide mas");
        assert!(ancho > 200.0);
        let mut ancha = e.clone();
        ancha.ancho = ancho;
        assert_eq!(ancho_para_los_nombres(&ancha), None, "con lo pedido sigue sin caber");
        assert!(textos(&ancha).iter().any(|(s, _, _)| s == "Estructura y muros de carga"));
        // Caso negativo: nombres cortos no piden nada.
        assert_eq!(ancho_para_los_nombres(&con_nombres(&["A", "B"], 400.0, Some(20.0))), None);
    }

    #[test]
    fn la_letra_y_su_tamano_viajan_en_el_fichero_como_en_el_movil() {
        let mut escena = crate::escena::Escena::nueva();
        escena.anadir(con_nombres(&["A"], 400.0, Some(18.0)));
        let lienzo = crate::excalidraw::con_escena(&crate::excalidraw::Lienzo::vacio(), &escena);
        let json = crate::excalidraw::escribir(&lienzo);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let el = &v["elements"][0];
        assert_eq!(el["type"], "pixpin-gantt");
        assert_eq!(el["fontSize"], 18.0);
        assert!(el["fontFamily"].is_u64());
        let vuelta = crate::excalidraw::a_escena(&crate::excalidraw::leer(&json).unwrap());
        let e = &vuelta.elementos[0];
        assert_eq!(e.extras.tam_letra, Some(18.0));
        assert_eq!(e.extras.familia.as_deref(), Some("Excalifont"));
    }
}
