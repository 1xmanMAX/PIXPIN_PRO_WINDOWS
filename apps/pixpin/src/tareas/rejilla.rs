//! **La vista de Tareas en tarjetas cuadradas** (tareas-v4): la logica pura.
//!
//! El usuario (5-oct): «que en la ventana de tareas se tenga dos
//! visualizaciones: uno como la actual en lista y dos en formato de
//! tarjetas cuadradas, de forma que si pego una foto la foto este dentro
//! como fondo del cuadrado [...] y si tiene varias fotos que aparezca como
//! si en el borde hubiera mas bordes simbolizando que hay mas tarjetas
//! detras».
//!
//! Aqui se decide donde va cada tarjeta y cada cosa dentro de ella; la
//! ventana pinta Y apunta los clics con estas mismas cajas (regla del
//! proyecto: una sola disposicion para las dos cosas, asi el clic nunca cae
//! al lado de lo pintado). Los grupos son los mismos que los de la lista
//! ([`super::tarjetas::agrupar`]): cambiar de vista no cambia que se ve.

#![forbid(unsafe_code)]

use std::path::Path;

use pixpin_render::RectF;

use super::tarjetas::{Colocada, Grupo, Pieza};

// ------------------------------------------------------------------- vista

/// Como se ensenan las tareas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vista {
    /// Una tarjeta ancha por tarea, una debajo de otra (tareas-v3).
    #[default]
    Lista,
    /// Cuadrados en rejilla, con la foto de fondo.
    Tarjetas,
}

/// Donde se recuerda la vista elegida, en la carpeta de datos: una palabra.
const FICHERO_VISTA: &str = "tareas-vista.txt";

impl Vista {
    /// Lo que no se entiende (un fichero roto, de otra version) es la lista:
    /// la vista de siempre.
    pub fn de_texto(s: &str) -> Vista {
        match s.trim() {
            "tarjetas" => Vista::Tarjetas,
            _ => Vista::Lista,
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Vista::Lista => "lista",
            Vista::Tarjetas => "tarjetas",
        }
    }

    pub fn leer(raiz: &Path) -> Vista {
        std::fs::read_to_string(raiz.join(FICHERO_VISTA))
            .map(|s| Vista::de_texto(&s))
            .unwrap_or_default()
    }

    /// Recordarla es un detalle: si no se puede escribir, se dice en el
    /// registro y la ventana sigue con la elegida.
    pub fn guardar(self, raiz: &Path) {
        if let Err(e) = std::fs::write(raiz.join(FICHERO_VISTA), self.como_texto()) {
            tracing::warn!(?e, "tareas: no se pudo recordar la vista");
        }
    }
}

// ----------------------------------------------------------------- colocar

/// Las medidas de la rejilla, ya con la escala.
#[derive(Debug, Clone, Copy)]
pub struct Medidas {
    /// El ancho del contenido.
    pub ancho: f32,
    /// El lado que se quiere para cada tarjeta: el de verdad sale de
    /// repartir el ancho entre las columnas que caben.
    pub lado: f32,
    /// Entre tarjeta y tarjeta, en las dos direcciones.
    pub hueco: f32,
    pub encabezado: f32,
    pub pliegue: f32,
    pub entre_grupos: f32,
    /// Lo que se deja encima de cada bloque de tarjetas: ahi asoman los
    /// bordes de la pila de una tarea con varias fotos.
    pub respiro: f32,
    /// El alto de cada hecha, abierto su pliegue: una fila a todo lo ancho,
    /// no un cuadrado (ver [`disponer`]).
    pub fila_hecha: f32,
}

/// El hueco entre dos filas de hechas, en logicos (con la escala, el de
/// [`Medidas::hueco`] partido en dos).
const HUECO_HECHAS: f32 = 0.5;

/// Una pieza en su caja: `x` desde el borde del contenido, `y` desde lo
/// alto del contenido.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Celda {
    pub pieza: Pieza,
    pub caja: RectF,
}

/// Cuantas columnas caben (al menos una) y el lado que les toca: el ancho
/// repartido entre ellas, para que la rejilla llene la ventana sin un hueco
/// a la derecha.
pub fn columnas(ancho: f32, lado: f32, hueco: f32) -> (usize, f32) {
    let n = (((ancho + hueco) / (lado + hueco)).floor() as usize).max(1);
    let real = ((ancho - (n - 1) as f32 * hueco) / n as f32).max(1.0);
    (n, real)
}

/// Coloca los grupos uno tras otro, como la lista: su encabezado a todo lo
/// ancho, sus tarjetas en filas, y si tiene hechas su pliegue y, abiertas,
/// las hechas debajo. Devuelve las celdas y el alto total.
///
/// **Las hechas son filas**, no cuadrados (tareas-v5): un cuadrado entero
/// para algo ya tachado pesaba tanto como lo pendiente, y con el pliegue
/// abierto la rejilla se llenaba de pasado. Una fila de `fila_hecha` a todo
/// lo ancho dice «esto ya esta» y deja lo pendiente como lo que se mira.
///
/// **Cada tarjeta, del alto de lo suyo** (8-oct-2026, el usuario: «que no
/// todas ocupen el mismo espacio: el ancho limitado pero no el alto, un
/// contenedor que se adapte a la tarea [...] que no pueda crecer mas del
/// cuadrado de hoy»). `alto_de(lista, fila, lado)` dice cuanto pide cada
/// una; se queda entre la mitad del lado y el lado. Y se colocan como un
/// muro: cada una en la columna que va mas corta, asi no quedan huecos.
pub fn disponer(
    grupos: &[Grupo],
    abierta: &dyn Fn(usize) -> bool,
    alto_de: &mut dyn FnMut(usize, usize, f32) -> f32,
    m: &Medidas,
) -> (Vec<Celda>, f32) {
    let (cols, lado) = columnas(m.ancho, m.lado, m.hueco);
    let mut v = Vec::new();
    let mut y = 0.0;
    let ancha = |pieza: Pieza, y: f32, alto: f32| Celda {
        pieza,
        caja: RectF {
            x: 0.0,
            y,
            ancho: m.ancho,
            alto,
        },
    };
    let bloque = |alto_de: &mut dyn FnMut(usize, usize, f32) -> f32,
                  lista: usize,
                  filas: &[usize],
                  y: &mut f32,
                  v: &mut Vec<Celda>| {
        if filas.is_empty() {
            return;
        }
        *y += m.respiro;
        // Lo que lleva cada columna, desde lo alto del bloque.
        let mut columna = vec![0.0f32; cols];
        for &fi in filas {
            // Del alto de su texto, sin cortarlo (8-oct-2026, noche: «que no
            // se pierda el texto»); como poco, medio cuadrado.
            let alto = alto_de(lista, fi, lado).max(lado * 0.5);
            // La mas corta; a igualdad, la de mas a la izquierda.
            let col = (0..cols)
                .min_by(|a, b| {
                    columna[*a]
                        .partial_cmp(&columna[*b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            v.push(Celda {
                pieza: Pieza::Tarjeta(lista, fi),
                caja: RectF {
                    x: col as f32 * (lado + m.hueco),
                    y: *y + columna[col],
                    ancho: lado,
                    alto,
                },
            });
            columna[col] += alto + m.hueco;
        }
        *y += columna.iter().copied().fold(0.0, f32::max) - m.hueco;
    };
    for (k, g) in grupos.iter().enumerate() {
        if k > 0 {
            y += m.entre_grupos;
        }
        v.push(ancha(Pieza::Encabezado(g.lista), y, m.encabezado));
        y += m.encabezado;
        bloque(&mut *alto_de, g.lista, &g.arriba, &mut y, &mut v);
        if !g.hechas.is_empty() {
            if !g.arriba.is_empty() {
                y += m.hueco;
            }
            v.push(ancha(Pieza::Pliegue(g.lista), y, m.pliegue));
            y += m.pliegue;
            if abierta(g.lista) {
                let entre = m.hueco * HUECO_HECHAS;
                for (n, &fi) in g.hechas.iter().enumerate() {
                    if n > 0 {
                        y += entre;
                    }
                    // Del alto de su texto entero, como las de arriba.
                    let alto = alto_de(g.lista, fi, m.ancho).max(m.fila_hecha);
                    v.push(ancha(Pieza::Tarjeta(g.lista, fi), y, alto));
                    y += alto;
                }
            }
        }
    }
    (v, y)
}

/// Las celdas como piezas de la columna (su `y` y su alto): lo que usan el
/// orden del teclado y llevar la del foco a la vista, igual en las dos
/// vistas.
pub fn colocadas(celdas: &[Celda]) -> Vec<Colocada> {
    celdas
        .iter()
        .map(|c| Colocada {
            pieza: c.pieza,
            y: c.caja.y,
            alto: c.caja.alto,
        })
        .collect()
}

fn dentro(r: RectF, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto
}

/// La pieza bajo el punto `(x, y)` del contenido. Los huecos no son de
/// nadie.
pub fn pieza_en(celdas: &[Celda], x: f32, y: f32) -> Option<Pieza> {
    celdas.iter().find(|c| dentro(c.caja, x, y)).map(|c| c.pieza)
}

// ------------------------------------------------------------------ teclado

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direccion {
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
}

/// La tarjeta adonde va el foco con una flecha, por la rejilla.
///
/// Izquierda y derecha siguen el orden de lectura (al acabar una fila pasan
/// a la siguiente, como en la galeria); arriba y abajo van a la fila de
/// encima o de debajo, a la tarjeta mas cercana en horizontal (aunque sea de
/// otro grupo). Sin foco, la primera. `None` solo al subir desde la primera
/// fila: el foco sale a la caja de apuntar. En los bordes se queda.
pub fn vecina(celdas: &[Celda], actual: Option<(usize, usize)>, d: Direccion) -> Option<(usize, usize)> {
    let tarjetas: Vec<(usize, usize, RectF)> = celdas
        .iter()
        .filter_map(|c| match c.pieza {
            Pieza::Tarjeta(l, f) => Some((l, f, c.caja)),
            _ => None,
        })
        .collect();
    let primera = tarjetas.first().map(|&(l, f, _)| (l, f));
    let Some(i) = actual.and_then(|a| tarjetas.iter().position(|&(l, f, _)| (l, f) == a)) else {
        return primera;
    };
    let (l0, f0, r0) = tarjetas[i];
    let par = |k: usize| (tarjetas[k].0, tarjetas[k].1);
    match d {
        Direccion::Izquierda => Some(par(i.saturating_sub(1))),
        Direccion::Derecha => Some(par((i + 1).min(tarjetas.len() - 1))),
        Direccion::Arriba | Direccion::Abajo => {
            let abajo = d == Direccion::Abajo;
            // La fila de al lado: la `y` mas cercana por ese lado.
            let fila = tarjetas
                .iter()
                .map(|t| t.2.y)
                .filter(|&y| if abajo { y > r0.y + 0.5 } else { y < r0.y - 0.5 })
                .fold(None::<f32>, |m, y| {
                    Some(match m {
                        None => y,
                        Some(m) if abajo => m.min(y),
                        Some(m) => m.max(y),
                    })
                });
            let Some(fy) = fila else {
                return if abajo { Some((l0, f0)) } else { None };
            };
            tarjetas
                .iter()
                .filter(|t| (t.2.y - fy).abs() <= 0.5)
                .min_by(|a, b| {
                    (a.2.x - r0.x)
                        .abs()
                        .partial_cmp(&(b.2.x - r0.x).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|t| (t.0, t.1))
        }
    }
}

// ------------------------------------------------------------ una tarjeta

/// Donde va cada cosa dentro de una tarjeta, en las coordenadas de su caja.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Partes {
    /// Abajo a la izquierda: la casilla que tacha (objetivo de 40).
    pub casilla: RectF,
    /// Arriba a la derecha, con el raton encima o el foco: quitarla.
    pub aspa: RectF,
    /// A la izquierda del aspa, en el Inbox: «Mover a…».
    pub mover: RectF,
    /// Arriba a la izquierda: la pila de circulos de emoticonos.
    pub chapas: RectF,
    /// El hueco del texto. Con foto se pega abajo; sin foto, arriba.
    pub texto: RectF,
    /// La linea de abajo a la derecha (`+N`, o la edad): su borde derecho y
    /// su centro vertical.
    pub pie: (f32, f32),
}

/// Diametro de los circulos de emoticonos (como los estados de WeChat).
pub const CHAPA: f32 = 40.0;

/// Las partes de la tarjeta de caja `c`. `con_chapas`: lleva emoticonos, y
/// el texto empieza debajo de ellos.
pub fn partes(c: RectF, s: f32, con_chapas: bool) -> Partes {
    let lado = 40.0 * s;
    // Los botones de arriba, de 40: los de icono del v2.
    let aspa_l = 40.0 * s;
    let casilla = RectF {
        x: c.x + 4.0 * s,
        y: c.y + c.alto - 4.0 * s - lado,
        ancho: lado,
        alto: lado,
    };
    let aspa = RectF {
        x: c.x + c.ancho - 6.0 * s - aspa_l,
        y: c.y + 6.0 * s,
        ancho: aspa_l,
        alto: aspa_l,
    };
    let mover = RectF {
        x: aspa.x - 4.0 * s - aspa_l,
        ..aspa
    };
    let chapas = RectF {
        x: c.x + 10.0 * s,
        y: c.y + 10.0 * s,
        ancho: (c.ancho - 20.0 * s).max(0.0),
        alto: CHAPA * s,
    };
    let arriba = if con_chapas {
        chapas.y + chapas.alto + 8.0 * s
    } else {
        c.y + 14.0 * s
    };
    let abajo = casilla.y - 2.0 * s;
    let texto = RectF {
        x: c.x + 12.0 * s,
        y: arriba,
        ancho: (c.ancho - 24.0 * s).max(0.0),
        alto: (abajo - arriba).max(0.0),
    };
    Partes {
        casilla,
        aspa,
        mover,
        chapas,
        texto,
        pie: (c.x + c.ancho - 12.0 * s, casilla.y + lado / 2.0),
    }
}


/// Los bordes de detras de una tarea con varias fotos, del mas lejano al
/// mas cercano: uno con dos fotos, dos con tres o mas (mas no se distingue).
/// Cada uno es la tarjeta corrida arriba a la derecha, como una pila de
/// cartas. Las tareas ya pintan la pila con `v2::tarjeta::pila`; esta
/// cuenta se queda porque el timeline coloca con ella sus tarjetas.
pub fn pila(c: RectF, fotos: usize, s: f32) -> Vec<RectF> {
    let capas = fotos.saturating_sub(1).min(2);
    (1..=capas)
        .rev()
        .map(|k| RectF {
            x: c.x + 5.0 * s * k as f32,
            y: c.y - 5.0 * s * k as f32,
            ..c
        })
        .collect()
}

/// El texto recortado para que quepa en `max_alto` (lo mide `medir`, con
/// los renglones partidos como se pintan): si no cabe, lo mas largo que
/// quepa con «…» al final. Por letras, no por bytes.
pub fn recortar_a_renglones(texto: &str, max_alto: f32, medir: &mut dyn FnMut(&str) -> f32) -> String {
    if texto.is_empty() || medir(texto) <= max_alto {
        return texto.to_string();
    }
    let cortes: Vec<usize> = texto.char_indices().map(|(i, _)| i).collect();
    let con_puntos = |k: usize| format!("{}…", texto[..cortes[k]].trim_end());
    // El prefijo de `k` letras mas largo que cabe: a saltos.
    let (mut bajo, mut alto) = (0usize, cortes.len() - 1);
    while bajo < alto {
        let mitad = (bajo + alto).div_ceil(2);
        if medir(&con_puntos(mitad)) <= max_alto {
            bajo = mitad;
        } else {
            alto = mitad - 1;
        }
    }
    con_puntos(bajo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn grupo(lista: usize, arriba: &[usize], hechas: &[usize]) -> Grupo {
        Grupo {
            lista,
            arriba: arriba.to_vec(),
            hechas: hechas.to_vec(),
        }
    }

    const M: Medidas = Medidas {
        ancho: 628.0,
        lado: 200.0,
        hueco: 12.0,
        encabezado: 40.0,
        pliegue: 40.0,
        entre_grupos: 18.0,
        respiro: 8.0,
        fila_hecha: 44.0,
    };

    #[test]
    fn las_columnas_llenan_el_ancho_con_cuadrados_de_unos_200() {
        let (n, lado) = columnas(628.0, 200.0, 12.0);
        assert_eq!(n, 3);
        assert!((lado * 3.0 + 24.0 - 628.0).abs() < 0.01);
        assert_eq!(columnas(1000.0, 200.0, 12.0).0, 4);
        // Caso negativo: en una ventana estrecha no hay cero columnas, y el
        // cuadrado encoge.
        let (n, lado) = columnas(150.0, 200.0, 12.0);
        assert_eq!((n, lado), (1, 150.0));
    }

    #[test]
    fn disponer_pone_cuadrados_en_filas_bajo_el_encabezado_de_su_lista() {
        let g = [grupo(0, &[5, 6, 7, 8], &[9]), grupo(1, &[0], &[])];
        let (c, total) = disponer(&g, &|_| false, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M);
        let (_, lado) = columnas(M.ancho, M.lado, M.hueco);
        let piezas: Vec<Pieza> = c.iter().map(|c| c.pieza).collect();
        assert_eq!(
            piezas,
            [
                Pieza::Encabezado(0),
                Pieza::Tarjeta(0, 5),
                Pieza::Tarjeta(0, 6),
                Pieza::Tarjeta(0, 7),
                Pieza::Tarjeta(0, 8),
                Pieza::Pliegue(0),
                Pieza::Encabezado(1),
                Pieza::Tarjeta(1, 0),
            ]
        );
        // Cuadrados, tres por fila; la cuarta baja a la segunda fila.
        assert_eq!(c[1].caja.ancho, c[1].caja.alto);
        assert_eq!((c[1].caja.x, c[1].caja.y), (0.0, 48.0));
        assert_eq!(c[3].caja.x, 2.0 * (lado + 12.0));
        assert_eq!((c[4].caja.x, c[4].caja.y), (0.0, 48.0 + lado + 12.0));
        // El pliegue, a todo lo ancho tras las dos filas.
        assert_eq!(c[5].caja.y, 48.0 + 2.0 * lado + 12.0 + 12.0);
        assert_eq!(c[5].caja.ancho, M.ancho);
        assert_eq!(total, c[7].caja.y + lado);
        // Abierto, las hechas van en filas de 44 a todo lo ancho, no en
        // cuadrados.
        let (c2, total2) = disponer(&g, &|l| l == 0, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M);
        assert_eq!(c2[6].pieza, Pieza::Tarjeta(0, 9));
        assert_eq!(c2[6].caja, RectF { x: 0.0, y: c2[5].caja.y + 40.0, ancho: M.ancho, alto: 44.0 });
        assert_eq!(total2, total + 44.0);
        // Dos hechas: una debajo de otra, con medio hueco entre ellas.
        let g2 = [grupo(0, &[], &[1, 2])];
        let (c3, _) = disponer(&g2, &|_| true, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M);
        assert_eq!(c3[3].caja.y, c3[2].caja.y + 44.0 + 6.0);
        assert_ne!(c3[3].caja.ancho, c3[3].caja.alto, "una fila, no un cuadrado");
        // Caso negativo: plegado, lo hecho no es celda; sin grupos, nada.
        assert!(!piezas.contains(&Pieza::Tarjeta(0, 9)));
        assert_eq!(disponer(&[], &|_| true, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M), (vec![], 0.0));
    }

    #[test]
    fn el_clic_cae_en_la_tarjeta_que_se_pinto_y_los_huecos_no_son_de_nadie() {
        let g = [grupo(0, &[1, 2, 3, 4], &[])];
        let (c, _) = disponer(&g, &|_| false, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M);
        let (_, lado) = columnas(M.ancho, M.lado, M.hueco);
        assert_eq!(pieza_en(&c, 10.0, 10.0), Some(Pieza::Encabezado(0)));
        assert_eq!(pieza_en(&c, 1.0, 50.0), Some(Pieza::Tarjeta(0, 1)));
        assert_eq!(pieza_en(&c, lado + 13.0, 50.0), Some(Pieza::Tarjeta(0, 2)));
        assert_eq!(
            pieza_en(&c, 5.0, 48.0 + lado + 12.0 + 5.0),
            Some(Pieza::Tarjeta(0, 4))
        );
        // Caso negativo: el hueco entre columnas, el de entre filas, el sitio
        // vacio a la derecha de la ultima y fuera de todo.
        assert_eq!(pieza_en(&c, lado + 6.0, 50.0), None);
        assert_eq!(pieza_en(&c, 5.0, 48.0 + lado + 6.0), None);
        assert_eq!(pieza_en(&c, lado + 20.0, 48.0 + lado + 20.0), None);
        assert_eq!(pieza_en(&c, -1.0, 50.0), None);
        // Las piezas de columna conservan el orden y la altura.
        let col = colocadas(&c);
        assert_eq!(col[1].y, 48.0);
        assert_eq!(col[4].alto, lado);
    }

    #[test]
    fn las_flechas_se_mueven_por_la_rejilla() {
        use Direccion::*;
        // Dos grupos: el primero con 4 (3 + 1), el segundo con 2.
        let g = [grupo(0, &[0, 1, 2, 3], &[]), grupo(1, &[0, 1], &[])];
        let (c, _) = disponer(&g, &|_| false, &mut |_, _, l| if l > 300.0 { 0.0 } else { l }, &M);
        assert_eq!(vecina(&c, None, Abajo), Some((0, 0)), "sin foco, la primera");
        assert_eq!(vecina(&c, Some((0, 0)), Derecha), Some((0, 1)));
        assert_eq!(vecina(&c, Some((0, 2)), Derecha), Some((0, 3)), "pasa de fila");
        assert_eq!(vecina(&c, Some((0, 1)), Abajo), Some((0, 3)), "la mas cercana de abajo");
        assert_eq!(vecina(&c, Some((0, 2)), Abajo), Some((0, 3)));
        assert_eq!(vecina(&c, Some((0, 3)), Abajo), Some((1, 0)), "salta de grupo");
        assert_eq!(vecina(&c, Some((1, 1)), Arriba), Some((0, 3)));
        assert_eq!(vecina(&c, Some((0, 3)), Arriba), Some((0, 0)), "misma columna");
        assert_eq!(vecina(&c, Some((0, 1)), Izquierda), Some((0, 0)));
        // Caso negativo: por arriba de la primera fila no hay tarjeta (sale
        // a la caja); en los otros bordes se queda; sin tarjetas, nada.
        assert_eq!(vecina(&c, Some((0, 1)), Arriba), None);
        assert_eq!(vecina(&c, Some((0, 0)), Izquierda), Some((0, 0)));
        assert_eq!(vecina(&c, Some((1, 1)), Derecha), Some((1, 1)));
        assert_eq!(vecina(&c, Some((1, 1)), Abajo), Some((1, 1)));
        assert_eq!(vecina(&[], Some((0, 0)), Abajo), None);
    }

    #[test]
    fn las_partes_de_una_tarjeta_caben_dentro_y_no_se_pisan() {
        let c = RectF {
            x: 100.0,
            y: 50.0,
            ancho: 200.0,
            alto: 200.0,
        };
        let p = partes(c, 1.0, true);
        let dentro_de = |r: RectF| {
            r.x >= c.x && r.y >= c.y && r.x + r.ancho <= c.x + c.ancho && r.y + r.alto <= c.y + c.alto
        };
        for r in [p.casilla, p.aspa, p.mover, p.chapas, p.texto] {
            assert!(dentro_de(r), "{r:?}");
        }
        assert!(p.casilla.y >= p.texto.y + p.texto.alto, "la casilla, debajo del texto");
        assert!(p.texto.y >= p.chapas.y + p.chapas.alto, "el texto, debajo de las chapas");
        assert!(p.mover.x + p.mover.ancho <= p.aspa.x);
        assert!(p.casilla.ancho >= 40.0 && p.aspa.ancho >= 36.0);
        // Caso negativo: sin emoticonos el texto no deja su sitio vacio.
        assert!(partes(c, 1.0, false).texto.y < p.texto.y);
        // Al doble de escala, todo al doble.
        assert_eq!(partes(c, 2.0, true).casilla.ancho, 80.0);
    }

    #[test]
    fn la_pila_tiene_un_borde_por_foto_de_mas_hasta_dos() {
        let c = RectF {
            x: 0.0,
            y: 20.0,
            ancho: 200.0,
            alto: 200.0,
        };
        assert!(pila(c, 0, 1.0).is_empty());
        assert!(pila(c, 1, 1.0).is_empty(), "caso negativo: una foto no es pila");
        let dos = pila(c, 2, 1.0);
        assert_eq!(dos.len(), 1);
        assert_eq!((dos[0].x, dos[0].y), (5.0, 15.0));
        let muchas = pila(c, 7, 1.0);
        assert_eq!(muchas.len(), 2);
        // El mas lejano primero (se pinta debajo).
        assert_eq!((muchas[0].x, muchas[0].y), (10.0, 10.0));
        assert_eq!(muchas[1].ancho, 200.0);
    }

    #[test]
    fn el_texto_largo_se_corta_con_puntos_suspensivos() {
        // Diez letras por renglon de 20 de alto.
        let mut medir = |t: &str| (t.chars().count() as f32 / 10.0).ceil() * 20.0;
        assert_eq!(recortar_a_renglones("corto", 40.0, &mut medir), "corto");
        let largo = "ñandú come mucho pan con queso cada mañana";
        let r = recortar_a_renglones(largo, 40.0, &mut medir);
        assert!(r.ends_with('…'), "{r}");
        assert!(r.chars().count() <= 20, "{r}");
        assert!(largo.starts_with(r.trim_end_matches('…')));
        // Caso negativo: lo vacio sigue vacio, y si no cabe ni una letra
        // queda solo «…», sin partir una letra por la mitad.
        assert_eq!(recortar_a_renglones("", 0.0, &mut medir), "");
        assert_eq!(recortar_a_renglones("ñu", 0.0, &mut medir), "…");
    }

    #[test]
    fn la_vista_se_lee_de_una_palabra_y_lo_raro_es_la_lista() {
        assert_eq!(Vista::de_texto("tarjetas\n"), Vista::Tarjetas);
        assert_eq!(Vista::de_texto(Vista::Lista.como_texto()), Vista::Lista);
        assert_eq!(Vista::de_texto(Vista::Tarjetas.como_texto()), Vista::Tarjetas);
        let raiz = std::env::temp_dir().join(format!("pixpin-tareas-vista-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        Vista::Tarjetas.guardar(&raiz);
        assert_eq!(Vista::leer(&raiz), Vista::Tarjetas);
        // Caso negativo: sin fichero o con basura, la lista.
        assert_eq!(Vista::de_texto("rejilla"), Vista::Lista);
        let _ = std::fs::remove_dir_all(&raiz);
        assert_eq!(Vista::leer(&raiz), Vista::Lista);
    }
}
