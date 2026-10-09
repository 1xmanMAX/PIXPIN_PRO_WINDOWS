//! **Las tarjetas de lecciones** dentro del timeline (5-oct-2026), en
//! funciones puras: el emoticono y el color de cada gravedad, la rejilla y
//! donde va cada boton de una tarjeta, y la busqueda en lecciones.
//!
//! El usuario: «mejora la parte de lecciones aprendidas, tiene muchos
//! elementos, quita la barra lateral; usa mas los emoticones para
//! representar el estado leve/medio/grave: emoticones y color a la vez,
//! grave rojo pero el emoticon pintado de rojo; lo de "me volvio a pasar"
//! esta bien, mantenlo; que este en formato de tarjetas […] y que timeline
//! y lecciones aprendidas las juntes». Por eso viven aqui, como cuarta
//! pestana, y no en su ventana de antes.
//!
//! Las mismas cajas sirven para pintar y para el clic (regla del proyecto).

use pixpin_lecciones::Leccion;
use pixpin_render::{Color, RectF};

use crate::lecciones::ui::v2;

/// Como se ensena una gravedad: el emoticono (que se pinta en monocromo,
/// del color), el color y la clave de su nombre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gravedad {
    pub emoticono: &'static str,
    pub color: Color,
    pub clave: &'static str,
}

/// 1 leve 😌 verde, 2 importante 😟 naranja, 3 grave 😡 rojo. Un valor raro
/// (de otro aparato) cuenta como el extremo mas cercano, como en la lista.
pub fn gravedad(g: i64) -> Gravedad {
    match g {
        i64::MIN..=1 => Gravedad {
            emoticono: "😌",
            color: v2::VERDE,
            clave: "timeline-gravedad-leve",
        },
        2 => Gravedad {
            emoticono: "😟",
            color: v2::NARANJA,
            clave: "timeline-gravedad-importante",
        },
        _ => Gravedad {
            emoticono: "😡",
            color: v2::ROJO,
            clave: "timeline-gravedad-grave",
        },
    }
}

/// Un clic en la gravedad le da la vuelta: leve, importante, grave, leve.
pub fn siguiente_gravedad(g: i64) -> i64 {
    match g {
        i64::MIN..=1 => 2,
        2 => 3,
        _ => 1,
    }
}

/// Lo menos que mide de ancho una tarjeta antes de pasar a menos columnas.
pub const ANCHO_MIN: f32 = 250.0;
pub const ALTO: f32 = 260.0;
pub const HUECO: f32 = 14.0;
pub const MARGEN: f32 = 16.0;
/// Lo de arriba de la rejilla: la pista.
pub const ARRIBA: f32 = 56.0;
const PAD: f32 = 14.0;

/// Cuantas columnas caben: de una a cuatro.
pub fn columnas(ancho: f32, s: f32) -> usize {
    let libre = ancho - 2.0 * MARGEN * s;
    (((libre + HUECO * s) / ((ANCHO_MIN + HUECO) * s)).floor() as usize).clamp(1, 4)
}

/// Las cajas de `n` tarjetas (en `y` desde lo alto del contenido) y el alto
/// de todo.
pub fn rejilla(n: usize, ancho: f32, s: f32) -> (Vec<RectF>, f32) {
    let cols = columnas(ancho, s);
    let libre = ancho - 2.0 * MARGEN * s;
    let w = (libre - (cols - 1) as f32 * HUECO * s) / cols as f32;
    let v: Vec<RectF> = (0..n)
        .map(|k| RectF {
            x: MARGEN * s + (k % cols) as f32 * (w + HUECO * s),
            y: ARRIBA * s + (k / cols) as f32 * (ALTO + HUECO) * s,
            ancho: w,
            alto: ALTO * s,
        })
        .collect();
    let filas = n.div_ceil(cols);
    (v, ARRIBA * s + filas as f32 * (ALTO + HUECO) * s + 60.0 * s)
}

/// Donde va cada cosa dentro de una tarjeta.
#[derive(Debug, Clone, PartialEq)]
pub struct Partes {
    /// La tarjeta en si: dentro de su celda, y mas pequena si lleva la pila
    /// de varias fotos (que asoma en el hueco y nunca fuera de la celda).
    pub tarjeta: RectF,
    /// Los bordes de detras con varias fotos (`tareas::rejilla::pila`), del
    /// mas lejano al mas cercano.
    pub pila: Vec<RectF>,
    /// La franja de arriba, igual en todas (asi los titulos alinean): la
    /// foto de portada o, sin foto, el color de su gravedad con su
    /// emoticono.
    pub banda: RectF,
    /// La chapita de la gravedad, arriba a la izquierda.
    pub gravedad: RectF,
    /// Donde va el texto (titulo y «la proxima vez»).
    pub texto: RectF,
    /// «↻ +1», abajo a la izquierda.
    pub otra_vez: RectF,
    /// Compartir como imagen, abajo a la derecha.
    pub compartir: RectF,
}

/// Lo que mide la franja de arriba.
pub const BANDA: f32 = 72.0;

pub fn partes(celda: RectF, fotos: usize, ancho_otra_vez: f32, s: f32) -> Partes {
    let pad = PAD * s;
    // Con varias fotos, la tarjeta deja sitio arriba y a la derecha para
    // los bordes de detras.
    let hueco = if fotos > 1 { 10.0 * s } else { 0.0 };
    let tarjeta = RectF {
        x: celda.x,
        y: celda.y + hueco,
        ancho: celda.ancho - hueco,
        alto: celda.alto - hueco,
    };
    let pila = crate::tareas::rejilla::pila(tarjeta, fotos, s);
    let banda = RectF {
        alto: BANDA * s,
        ..tarjeta
    };
    let gravedad = RectF {
        x: tarjeta.x + 10.0 * s,
        y: tarjeta.y + 10.0 * s,
        ancho: 34.0 * s,
        alto: 30.0 * s,
    };
    let abajo = 36.0 * s;
    let otra_vez = RectF {
        x: tarjeta.x + pad - 4.0 * s,
        y: tarjeta.y + tarjeta.alto - pad + 4.0 * s - abajo,
        ancho: ancho_otra_vez,
        alto: abajo,
    };
    let compartir = RectF {
        x: tarjeta.x + tarjeta.ancho - pad + 4.0 * s - abajo,
        y: otra_vez.y,
        ancho: abajo,
        alto: abajo,
    };
    let texto_y = banda.y + banda.alto + 12.0 * s;
    Partes {
        tarjeta,
        pila,
        banda,
        gravedad,
        texto: RectF {
            x: tarjeta.x + pad,
            y: texto_y,
            ancho: tarjeta.ancho - 2.0 * pad,
            alto: (otra_vez.y - 8.0 * s - texto_y).max(0.0),
        },
        otra_vez,
        compartir,
    }
}

/// Como se ordenan las tarjetas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orden {
    /// Las tocadas hace menos, primero.
    Recientes,
    /// Las que mas veces pasaron, primero.
    MasRepetidas,
}

/// Las lecciones (sus indices) que se ven: las de `consulta` y de la
/// gravedad elegida (`None` = todas), en el orden pedido.
pub fn filtrar(lecciones: &[&Leccion], consulta: &str, gravedad: Option<i64>, orden: Orden) -> Vec<usize> {
    let mut v: Vec<usize> = buscar(lecciones, consulta)
        .into_iter()
        .filter(|&k| gravedad.is_none_or(|g| self::gravedad(lecciones[k].gravedad) == self::gravedad(g)))
        .collect();
    match orden {
        Orden::Recientes => v.sort_by_key(|&k| std::cmp::Reverse(lecciones[k].tocada)),
        Orden::MasRepetidas => v.sort_by_key(|&k| {
            (
                std::cmp::Reverse(lecciones[k].veces_que_paso()),
                std::cmp::Reverse(lecciones[k].tocada),
            )
        }),
    }
    v
}

/// Cuantas hay de cada gravedad (leve, importante, grave): los numeros de
/// los chips de filtro.
pub fn cuantas_por_gravedad(lecciones: &[&Leccion]) -> [usize; 3] {
    let mut c = [0; 3];
    for l in lecciones {
        let k = match self::gravedad(l.gravedad).emoticono {
            "😌" => 0,
            "😟" => 1,
            _ => 2,
        };
        c[k] += 1;
    }
    c
}

/// Las lecciones (sus indices) donde esta cada palabra de `consulta`, sin
/// tildes ni mayusculas: en el titulo, los tres campos y las etiquetas.
pub fn buscar(lecciones: &[&Leccion], consulta: &str) -> Vec<usize> {
    let palabras: Vec<String> = pixpin_timeline::buscar::normalizar(consulta)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if palabras.is_empty() {
        return (0..lecciones.len()).collect();
    }
    lecciones
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let donde = pixpin_timeline::buscar::normalizar(&format!(
                "{}\n{}\n{}\n{}\n{}",
                l.titulo,
                l.que_paso,
                l.por_que,
                l.proxima,
                l.todas_las_etiquetas().join(" ")
            ));
            palabras.iter().all(|p| donde.contains(p.as_str()))
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventanita::dentro;

    #[test]
    fn cada_gravedad_lleva_su_emoticono_y_su_color_a_la_vez() {
        assert_eq!(gravedad(1).emoticono, "😌");
        assert_eq!(gravedad(1).color, v2::VERDE);
        assert_eq!(gravedad(2).emoticono, "😟");
        assert_eq!(gravedad(2).color, v2::NARANJA);
        assert_eq!(gravedad(3).emoticono, "😡");
        assert_eq!(gravedad(3).color, v2::ROJO);
        // Caso negativo: un valor raro no se queda sin cara.
        assert_eq!(gravedad(0), gravedad(1));
        assert_eq!(gravedad(9), gravedad(3));
        assert_ne!(gravedad(1).color, gravedad(3).color);
    }

    #[test]
    fn un_clic_da_la_vuelta_a_la_gravedad() {
        assert_eq!(siguiente_gravedad(1), 2);
        assert_eq!(siguiente_gravedad(2), 3);
        assert_eq!(siguiente_gravedad(3), 1);
        assert_eq!(siguiente_gravedad(7), 1);
    }

    #[test]
    fn la_rejilla_pone_mas_columnas_cuanto_mas_ancha_sin_salirse() {
        assert_eq!(columnas(900.0, 1.0), 3);
        assert_eq!(columnas(400.0, 1.0), 1);
        assert_eq!(columnas(3000.0, 1.0), 4);
        let (v, alto) = rejilla(7, 900.0, 1.0);
        assert_eq!(v.len(), 7);
        assert_eq!(v[3].x, v[0].x);
        assert!(v[3].y > v[0].y);
        for r in &v {
            assert!(r.x + r.ancho <= 900.0 - MARGEN + 0.5);
        }
        assert!(alto > v[6].y + v[6].alto);
    }

    #[test]
    fn caso_negativo_los_botones_de_una_tarjeta_no_se_pisan() {
        let (v, _) = rejilla(1, 900.0, 1.0);
        for fotos in [0, 1, 3] {
            let p = partes(v[0], fotos, 150.0, 1.0);
            let centro = |r: RectF| (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
            assert!(!dentro(p.otra_vez, centro(p.compartir)));
            assert!(!dentro(p.compartir, centro(p.otra_vez)));
            // Todo dentro de la tarjeta.
            for r in [p.gravedad, p.otra_vez, p.compartir] {
                assert!(dentro(v[0], (r.x, r.y)) && dentro(v[0], (r.x + r.ancho, r.y + r.alto)));
            }
            assert!(p.texto.y + p.texto.alto <= p.otra_vez.y);
            // La pila de varias fotos asoma dentro de la celda, nunca por
            // encima de ella.
            for c in &p.pila {
                assert!(c.y >= v[0].y - 0.5 && c.x + c.ancho <= v[0].x + v[0].ancho + 0.5);
            }
            assert_eq!(p.pila.len(), fotos.saturating_sub(1).min(2));
        }
    }

    #[test]
    fn los_filtros_por_gravedad_y_el_orden_por_repeticiones() {
        let mut a = Leccion::nueva("a", 1, "Uno");
        a.gravedad = 3;
        a.tocada = 10;
        let mut b = Leccion::nueva("b", 2, "Dos");
        b.gravedad = 1;
        b.tocada = 20;
        b.repeticiones = vec![1, 2];
        let mut c = Leccion::nueva("c", 3, "Tres");
        c.gravedad = 3;
        c.tocada = 30;
        let v = [&a, &b, &c];
        assert_eq!(filtrar(&v, "", None, Orden::Recientes), [2, 1, 0]);
        assert_eq!(filtrar(&v, "", None, Orden::MasRepetidas), [1, 2, 0]);
        assert_eq!(filtrar(&v, "", Some(3), Orden::Recientes), [2, 0]);
        assert_eq!(cuantas_por_gravedad(&v), [1, 0, 2]);
        // Caso negativo: una gravedad sin ninguna no da nada.
        assert!(filtrar(&v, "", Some(2), Orden::Recientes).is_empty());
    }

    #[test]
    fn la_busqueda_mira_titulo_campos_y_etiquetas_sin_tildes() {
        let mut a = Leccion::nueva("a", 1, "Revisar el andamio");
        a.por_que = "Prisa por acabar".into();
        let mut b = Leccion::nueva("b", 2, "Comprar más yeso");
        b.etiquetas = vec!["obra".into()];
        let v = [&a, &b];
        assert_eq!(buscar(&v, "prisa"), [0]);
        assert_eq!(buscar(&v, "MAS obra"), [1]);
        // Caso negativo: lo que no esta no sale; sin consulta salen todas.
        assert!(buscar(&v, "piscina").is_empty());
        assert_eq!(buscar(&v, "  "), [0, 1]);
    }
}
