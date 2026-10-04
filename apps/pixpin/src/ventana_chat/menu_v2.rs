//! **Los menus del chat, v2** (`Menus2.dc.html`, aprobado el 3-oct).
//!
//! Lo que cambia respecto al menu de Telegram de antes:
//!
//! - cada entrada lleva su icono y, a la derecha, la **chapita de su tecla**
//!   («R», «Ctrl C», «Supr»…), que vale con el menu abierto;
//! - las entradas se agrupan con **separadores**: lo diario arriba, lo raro
//!   en «Más» (un submenu al lado) y **Borrar en rojo y apartado**;
//! - el menu de un mensaje lleva encima la **fila de etiquetas** (⭐✅⏳…);
//! - filas de 40 px y letra de 14 px, que es lo que pide la v2.
//!
//! Aqui va la geometria pura y las teclas, que se prueban sin ventana; el
//! pintado va al final y usa las mismas cuentas.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
pub(super) const FILA: u32 = 40;
pub(super) const SEPARADOR: u32 = 11;
pub(super) const RELLENO: u32 = 6;
pub(super) const RADIO: u32 = 14;
pub(super) const RADIO_FILA: u32 = 9;
pub(super) const TAM: f32 = 14.0;
pub(super) const TAM_CHAPA: f32 = 11.0;
pub(super) const ICONO: u32 = 18;
pub(super) const ICONO_X: u32 = 10;
pub(super) const TEXTO_X: u32 = 40;
pub(super) const DERECHA: u32 = 10;
/// Entre el rotulo y sus chapitas, para que no se lean pegados.
pub(super) const HUECO_CHAPA: u32 = 18;
pub(super) const CHAPA_ALTO: u32 = 20;
pub(super) const CHAPA_MIN: u32 = 18;
pub(super) const CHAPA_HUECO: u32 = 3;
pub(super) const ANCHO_MIN: u32 = 220;
pub(super) const ANCHO_MAX: u32 = 400;
/// La fila de etiquetas: circulos de 40 px.
pub(super) const EMOJI: u32 = 40;
pub(super) const EMOJI_HUECO: u32 = 6;
pub(super) const EMOJI_BAJO: u32 = 8;
/// Cuanto se monta el submenu «Más» sobre su madre.
pub(super) const SOLAPE: u32 = 6;

/// La tecla de una entrada, que es a la vez lo que dice su chapita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tecla {
    /// Una letra sola: solo vale con el menu abierto.
    Letra(char),
    /// Ctrl y una letra.
    Ctrl(char),
    Supr,
    F2,
    /// Solo se ensena («Ctrl clic» para elegir varios): no es una tecla.
    CtrlClic,
    /// La flecha de un submenu: tampoco es una tecla.
    Sub,
}

const VK_SUPR: u32 = 0x2E;
const VK_F2: u32 = 0x71;

impl Tecla {
    /// Si esta tecla es la de esa pulsacion (`WM_KEYDOWN`). Las letras
    /// solas NO se miran aqui: llegan como caracter (`coincide_caracter`),
    /// que es lo que respeta la distribucion del teclado.
    pub(super) fn coincide_tecla(self, vk: u32, ctrl: bool) -> bool {
        match self {
            Tecla::Ctrl(c) => ctrl && vk == c.to_ascii_uppercase() as u32,
            Tecla::Supr => !ctrl && vk == VK_SUPR,
            Tecla::F2 => !ctrl && vk == VK_F2,
            _ => false,
        }
    }

    /// Si esta tecla es ese caracter escrito, sin mirar mayusculas.
    pub(super) fn coincide_caracter(self, c: char) -> bool {
        match self {
            Tecla::Letra(l) => l.to_lowercase().eq(c.to_lowercase()),
            _ => false,
        }
    }

    /// Las chapitas que se pintan, en orden. `supr` es como se llama la
    /// tecla en el idioma (Supr / Del); `clic`, el clic.
    pub(super) fn chapas(self, supr: &str, clic: &str) -> Vec<String> {
        match self {
            Tecla::Letra(c) => vec![c.to_uppercase().collect()],
            Tecla::Ctrl(c) => vec!["Ctrl".into(), c.to_uppercase().collect()],
            Tecla::Supr => vec![supr.into()],
            Tecla::F2 => vec!["F2".into()],
            Tecla::CtrlClic => vec!["Ctrl".into(), clic.into()],
            Tecla::Sub => Vec::new(),
        }
    }
}

/// Si en esas teclas hay alguna repetida (lo que de verdad se pulsa: las
/// chapitas de solo ensenar no cuentan). Un menu con dos «R» haria lo
/// primero que encontrara, que no es lo que se lee.
#[cfg(test)]
pub(super) fn hay_repetidas(teclas: &[Tecla]) -> bool {
    let reales: Vec<Tecla> = teclas
        .iter()
        .copied()
        .filter(|t| !matches!(t, Tecla::CtrlClic | Tecla::Sub))
        .map(|t| match t {
            Tecla::Letra(c) => Tecla::Letra(c.to_ascii_lowercase()),
            Tecla::Ctrl(c) => Tecla::Ctrl(c.to_ascii_lowercase()),
            otra => otra,
        })
        .collect();
    reales.iter().enumerate().any(|(n, t)| reales[..n].contains(t))
}

/// Un menu (o su submenu) ya colocado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Colocado {
    /// El recuadro de las entradas.
    pub caja: Rect,
    pub filas: Vec<Rect>,
    /// La `y` de cada raya de separar.
    pub separadores: Vec<i32>,
    /// Los circulos de la fila de etiquetas, encima del recuadro.
    pub emojis: Vec<Rect>,
}

fn e(v: u32, escala: u32) -> u32 {
    v * escala / 100
}

/// Lo ancho y lo alto del recuadro, dado lo que necesita cada fila (su
/// rotulo y sus chapitas, ya medidos con la fuente).
pub(super) fn medidas(necesita: &[f32], separado: &[bool], escala: u32) -> (u32, u32) {
    let mas_ancha = necesita.iter().fold(0.0f32, |a, b| a.max(*b)).max(0.0).ceil() as u32;
    let natural = mas_ancha + e(TEXTO_X + DERECHA + 2 * RELLENO, escala);
    let ancho = natural.clamp(e(ANCHO_MIN, escala), e(ANCHO_MAX, escala));
    let seps = separado
        .iter()
        .enumerate()
        .filter(|(n, s)| **s && *n > 0)
        .count() as u32;
    let alto = e(FILA, escala) * necesita.len() as u32 + e(SEPARADOR, escala) * seps + 2 * e(RELLENO, escala);
    (ancho, alto)
}

/// Lo que ocupa la fila de etiquetas: ancho y alto (con su hueco de debajo).
fn medidas_emojis(cuantos: usize, escala: u32) -> (u32, u32) {
    if cuantos == 0 {
        return (0, 0);
    }
    let n = cuantos as u32;
    (
        e(EMOJI, escala) * n + e(EMOJI_HUECO, escala) * (n - 1),
        e(EMOJI + EMOJI_BAJO, escala),
    )
}

/// Pone las filas dentro de un recuadro que empieza en `(x, y)`.
fn construir(x: i32, y: i32, ancho: u32, alto: u32, separado: &[bool], escala: u32) -> Colocado {
    let relleno = e(RELLENO, escala) as i32;
    let fila = e(FILA, escala);
    let sep = e(SEPARADOR, escala) as i32;
    let mut cursor = y + relleno;
    let mut filas = Vec::with_capacity(separado.len());
    let mut separadores = Vec::new();
    for (n, s) in separado.iter().enumerate() {
        if *s && n > 0 {
            separadores.push(cursor + sep / 2);
            cursor += sep;
        }
        filas.push(Rect {
            x: x + relleno,
            y: cursor,
            ancho: ancho.saturating_sub(2 * relleno as u32),
            alto: fila,
        });
        cursor += fila as i32;
    }
    Colocado {
        caja: Rect { x, y, ancho, alto },
        filas,
        separadores,
        emojis: Vec::new(),
    }
}

/// Coloca un menu que nace en `ancla` (donde se pulso): hacia abajo y a la
/// derecha si cabe; si no, hacia arriba o a la izquierda; y siempre dentro
/// de `limite`. Con `emojis`, la fila de etiquetas va encima y cuenta como
/// parte del menu al decidir si cabe.
pub(super) fn colocar(
    ancla: Punto,
    limite: Rect,
    necesita: &[f32],
    separado: &[bool],
    emojis: usize,
    escala: u32,
) -> Colocado {
    let (ancho, alto) = medidas(necesita, separado, escala);
    let (ancho_emo, alto_emo) = medidas_emojis(emojis, escala);
    let ancho_total = ancho.max(ancho_emo) as i32;
    let alto_total = (alto + alto_emo) as i32;
    let x = if ancla.x + ancho_total <= limite.derecha() {
        ancla.x
    } else {
        ancla.x - ancho_total
    };
    let y = if ancla.y + alto_total <= limite.abajo() {
        ancla.y
    } else {
        ancla.y - alto_total
    };
    let x = x.min(limite.derecha() - ancho_total).max(limite.x);
    let y = y.min(limite.abajo() - alto_total).max(limite.y);
    let mut c = construir(x, y + alto_emo as i32, ancho, alto, separado, escala);
    let lado = e(EMOJI, escala);
    let paso = (lado + e(EMOJI_HUECO, escala)) as i32;
    c.emojis = (0..emojis)
        .map(|n| Rect {
            x: x + n as i32 * paso,
            y,
            ancho: lado,
            alto: lado,
        })
        .collect();
    c
}

/// Coloca el submenu «Más» al lado de su fila: a la derecha de la madre si
/// cabe, si no a su izquierda; con su primera fila a la altura de la de
/// «Más» y sin salirse por abajo.
pub(super) fn colocar_al_lado(
    madre: Rect,
    fila: Rect,
    limite: Rect,
    necesita: &[f32],
    separado: &[bool],
    escala: u32,
) -> Colocado {
    let (ancho, alto) = medidas(necesita, separado, escala);
    let solape = e(SOLAPE, escala) as i32;
    let derecha = madre.derecha() - solape;
    let x = if derecha + ancho as i32 <= limite.derecha() {
        derecha
    } else {
        madre.x + solape - ancho as i32
    };
    let x = x.min(limite.derecha() - ancho as i32).max(limite.x);
    let y = (fila.y - e(RELLENO, escala) as i32)
        .min(limite.abajo() - alto as i32)
        .max(limite.y);
    construir(x, y, ancho, alto, separado, escala)
}

impl Colocado {
    /// Que fila hay bajo el punto. El relleno y los separadores no son de
    /// ninguna.
    pub(super) fn fila_en(&self, p: Punto) -> Option<usize> {
        self.filas.iter().position(|f| f.contiene(p))
    }

    pub(super) fn emoji_en(&self, p: Punto) -> Option<usize> {
        self.emojis.iter().position(|r| r.contiene(p))
    }

    /// Si el punto cae en el menu (recuadro o fila de etiquetas), aunque
    /// no sea en ninguna entrada: ahi un clic no lo cierra.
    pub(super) fn contiene(&self, p: Punto) -> bool {
        self.caja.contiene(p) || self.emojis.iter().any(|r| r.contiene(p))
    }
}

/// Que cosa del menu esta bajo el raton o elegida con las flechas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sitio {
    Principal(usize),
    Mas(usize),
    Emoji(usize),
}

/// La siguiente entrada al ir con las flechas, dando la vuelta.
pub(super) fn siguiente(actual: Option<usize>, cuantas: usize, abajo: bool) -> Option<usize> {
    if cuantas == 0 {
        return None;
    }
    Some(match (actual, abajo) {
        (None, true) => 0,
        (None, false) => cuantas - 1,
        (Some(n), true) => (n + 1) % cuantas,
        (Some(n), false) => (n + cuantas - 1) % cuantas,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn limite() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1000,
            alto: 800,
        }
    }

    #[test]
    fn las_filas_miden_40_y_los_separadores_las_apartan() {
        let separado = [false, false, true, false, true];
        let c = colocar(Punto { x: 100, y: 100 }, limite(), &[80.0; 5], &separado, 0, 100);
        assert_eq!(c.filas.len(), 5);
        assert!(c.filas.iter().all(|f| f.alto == FILA));
        assert_eq!(c.separadores.len(), 2);
        // La fila 2 empieza un separador mas abajo que donde acaba la 1.
        assert_eq!(c.filas[2].y, c.filas[1].abajo() + SEPARADOR as i32);
        // Y la 1 justo donde acaba la 0.
        assert_eq!(c.filas[1].y, c.filas[0].abajo());
        // Todo dentro del recuadro.
        assert!(c.filas.last().unwrap().abajo() <= c.caja.abajo());
        // Caso negativo: un separador en la primera fila no se pinta.
        let c = colocar(Punto { x: 0, y: 0 }, limite(), &[80.0; 2], &[true, false], 0, 100);
        assert!(c.separadores.is_empty());
        assert_eq!(c.filas[0].y, RELLENO as i32);
    }

    #[test]
    fn el_ancho_va_del_minimo_al_maximo() {
        let (a, _) = medidas(&[10.0], &[false], 100);
        assert_eq!(a, ANCHO_MIN);
        let (a, _) = medidas(&[2000.0], &[false], 100);
        assert_eq!(a, ANCHO_MAX);
        let (a, _) = medidas(&[250.0], &[false], 100);
        assert_eq!(a, 250 + TEXTO_X + DERECHA + 2 * RELLENO);
    }

    #[test]
    fn junto_al_borde_se_da_la_vuelta_sin_salirse() {
        let ancla = Punto { x: 990, y: 790 };
        let c = colocar(ancla, limite(), &[100.0; 6], &[false; 6], 6, 100);
        assert!(c.caja.derecha() <= 1000 && c.caja.abajo() <= 800, "{:?}", c.caja);
        // La fila de etiquetas va encima del recuadro, sin pisarlo.
        assert_eq!(c.emojis.len(), 6);
        assert!(c.emojis.iter().all(|r| r.abajo() <= c.caja.y));
        assert!(c.emojis.iter().all(|r| r.y >= 0));
        // Caso negativo: lejos del borde nace en el ancla misma.
        let c = colocar(Punto { x: 50, y: 60 }, limite(), &[100.0], &[false], 0, 100);
        assert_eq!((c.caja.x, c.caja.y), (50, 60));
    }

    #[test]
    fn se_acierta_la_fila_y_el_relleno_no_es_ninguna() {
        let c = colocar(Punto { x: 0, y: 0 }, limite(), &[80.0; 3], &[false, true, false], 3, 100);
        let f = c.filas[1];
        assert_eq!(c.fila_en(Punto { x: f.x + 4, y: f.y + 4 }), Some(1));
        assert_eq!(c.emoji_en(Punto { x: c.emojis[2].x + 2, y: c.emojis[2].y + 2 }), Some(2));
        // Casos negativos: el relleno de arriba y la raya de separar.
        let arriba = Punto { x: c.caja.x + 3, y: c.caja.y + 2 };
        assert_eq!(c.fila_en(arriba), None);
        assert!(c.contiene(arriba), "el relleno es del menu: no lo cierra");
        let raya = Punto { x: f.x + 4, y: c.separadores[0] };
        assert_eq!(c.fila_en(raya), None);
        assert!(!c.contiene(Punto { x: 999, y: 799 }));
    }

    #[test]
    fn el_submenu_va_a_la_derecha_o_a_la_izquierda_si_no_cabe() {
        let madre = Rect { x: 100, y: 100, ancho: 300, alto: 400 };
        let fila = Rect { x: 106, y: 300, ancho: 288, alto: 40 };
        let s = colocar_al_lado(madre, fila, limite(), &[100.0; 4], &[false; 4], 100);
        assert!(s.caja.x > madre.x && s.caja.x < madre.derecha(), "se monta un poco");
        assert_eq!(s.filas[0].y, fila.y, "su primera fila a la altura de «Más»");
        // Caso negativo: pegada al borde derecho, sale por la izquierda.
        let madre = Rect { x: 690, ..madre };
        let s = colocar_al_lado(madre, fila, limite(), &[100.0; 4], &[false; 4], 100);
        assert!(s.caja.derecha() <= madre.x + SOLAPE as i32);
        assert!(s.caja.x >= 0);
    }

    #[test]
    fn las_teclas_coinciden_con_lo_que_dicen() {
        assert!(Tecla::Letra('r').coincide_caracter('R'));
        assert!(Tecla::Letra('r').coincide_caracter('r'));
        assert!(Tecla::Ctrl('c').coincide_tecla(0x43, true));
        assert!(Tecla::Supr.coincide_tecla(0x2E, false));
        assert!(Tecla::F2.coincide_tecla(0x71, false));
        // Casos negativos: la C sin Ctrl no es copiar, y la letra no es una
        // pulsacion de tecla (llega como caracter).
        assert!(!Tecla::Ctrl('c').coincide_tecla(0x43, false));
        assert!(!Tecla::Letra('r').coincide_tecla(0x52, false));
        assert!(!Tecla::Letra('r').coincide_caracter('p'));
        assert!(!Tecla::CtrlClic.coincide_tecla(0x01, true));
        assert_eq!(Tecla::Ctrl('c').chapas("Supr", "clic"), ["Ctrl", "C"]);
        assert!(Tecla::Sub.chapas("Supr", "clic").is_empty());
    }

    #[test]
    fn se_ven_las_teclas_repetidas() {
        assert!(!hay_repetidas(&[Tecla::Letra('r'), Tecla::Letra('p'), Tecla::Ctrl('c')]));
        // Las de solo ensenar no cuentan.
        assert!(!hay_repetidas(&[Tecla::Sub, Tecla::Sub, Tecla::CtrlClic]));
        // Caso negativo: dos R, aunque una sea mayuscula.
        assert!(hay_repetidas(&[Tecla::Letra('r'), Tecla::Letra('R')]));
    }

    #[test]
    fn las_flechas_dan_la_vuelta() {
        assert_eq!(siguiente(None, 3, true), Some(0));
        assert_eq!(siguiente(None, 3, false), Some(2));
        assert_eq!(siguiente(Some(2), 3, true), Some(0));
        assert_eq!(siguiente(Some(0), 3, false), Some(2));
        // Caso negativo: sin entradas no hay ninguna.
        assert_eq!(siguiente(Some(0), 0, true), None);
    }
}
