//! **Lo que la galeria decide sin pintar nada** (v2): el color de la
//! caducidad, la busqueda, como caen los grupos por dia y las celdas, y
//! adonde va el foco con las flechas. Los filtros del carril se fueron con
//! el carril (5-oct): el usuario no los usaba.
//!
//! Aparte de la ventana para poder probarlo todo sin GPU.

#![forbid(unsafe_code)]

use pixpin_render::RectF;

pub const DIA_MS: i64 = 86_400_000;

/// El dia LOCAL de un instante en ms UTC, contado desde 1970.
pub fn dia_local(utc_ms: i64) -> i64 {
    pixpin_shell::entorno::a_local(utc_ms).div_euclid(DIA_MS)
}

/// Dia de la semana de un dia desde 1970, con el lunes en 0. El 1-1-1970
/// fue jueves (3).
pub fn dia_de_la_semana(dia: i64) -> u32 {
    (dia + 3).rem_euclid(7) as u32
}

// ------------------------------------------------------------- caducidad

/// Dias de calendario (locales) que faltan de `ahora` a `se_va`.
pub fn dias_que_faltan(se_va: i64, ahora: i64) -> i64 {
    dia_local(se_va) - dia_local(ahora)
}

/// **Los dias que le quedan** a una captura: el numero de su chapita roja
/// (5-oct). 0 = se va hoy. `None` = conservada, no se va.
pub fn dias_que_quedan(se_va: Option<i64>, ahora: i64) -> Option<i64> {
    se_va.map(|t| dias_que_faltan(t, ahora).max(0))
}

// -------------------------------------------------------------- busqueda

/// En minusculas y sin tildes: «Árbol» encuentra «arbol» y al reves.
pub fn plegar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            c => c,
        })
        .collect()
}

/// Si TODAS las palabras de la consulta (ya plegada) estan en alguno de los
/// campos (ya plegados). Una consulta vacia encaja con todo.
pub fn encaja(consulta_plegada: &str, campos: &[&str]) -> bool {
    consulta_plegada
        .split_whitespace()
        .all(|palabra| campos.iter().any(|c| c.contains(palabra)))
}

// ------------------------------------------------------------- disposicion

/// Medidas, en pixeles logicos.
pub const CABECERA: f32 = 64.0;
pub const LADO_REJILLA: f32 = 20.0;
pub const CELDA_MIN: f32 = 180.0;
pub const CELDA_ALTO: f32 = 140.0;
pub const HUECO: f32 = 12.0;
pub const TITULO_DIA: f32 = 40.0;
/// Lo que se deja libre abajo para que la barra de elegidas no tape la
/// ultima fila.
pub const PIE: f32 = 96.0;

/// Un grupo de un dia.
#[derive(Debug, Clone, PartialEq)]
pub struct Grupo {
    pub dia: i64,
    /// Arriba del titulo, sin desplazar, relativo a lo alto de la rejilla.
    pub y: f32,
    /// Las posiciones (en la vista filtrada) que van en el.
    pub desde: usize,
    pub hasta: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub columnas: usize,
    pub grupos: Vec<Grupo>,
    /// Una por captura de la vista, relativas a la esquina de la rejilla y
    /// sin desplazar.
    pub celdas: Vec<RectF>,
    pub alto_total: f32,
}

/// Coloca las capturas (ya en orden, la mas nueva primero) en grupos por
/// dia y en filas de `columnas`. `dias[i]` es el dia local de la `i`.
pub fn disponer(dias: &[i64], ancho: f32, escala: f32) -> Disposicion {
    let lado = LADO_REJILLA * escala;
    let hueco = HUECO * escala;
    let util = (ancho - 2.0 * lado).max(CELDA_MIN * escala);
    let columnas = (((util + hueco) / (CELDA_MIN * escala + hueco)).floor() as usize).max(1);
    let ancho_celda = (util - hueco * (columnas as f32 - 1.0)) / columnas as f32;
    let alto_celda = CELDA_ALTO * escala;
    let mut grupos = Vec::new();
    let mut celdas = Vec::with_capacity(dias.len());
    let mut y = 8.0 * escala;
    let mut i = 0;
    while i < dias.len() {
        let dia = dias[i];
        let mut j = i;
        while j < dias.len() && dias[j] == dia {
            j += 1;
        }
        if !grupos.is_empty() {
            y += 8.0 * escala;
        }
        grupos.push(Grupo {
            dia,
            y,
            desde: i,
            hasta: j,
        });
        y += TITULO_DIA * escala;
        for k in i..j {
            let n = k - i;
            let (fila, col) = (n / columnas, n % columnas);
            celdas.push(RectF {
                x: lado + col as f32 * (ancho_celda + hueco),
                y: y + fila as f32 * (alto_celda + hueco),
                ancho: ancho_celda,
                alto: alto_celda,
            });
        }
        let filas = (j - i).div_ceil(columnas);
        y += filas as f32 * (alto_celda + hueco);
        i = j;
    }
    Disposicion {
        columnas,
        grupos,
        celdas,
        alto_total: y + PIE * escala,
    }
}

impl Disposicion {
    pub fn scroll_maximo(&self, alto_vista: f32) -> f32 {
        (self.alto_total - alto_vista).max(0.0)
    }

    /// Las posiciones cuyas celdas asoman entre `y0` y `y1` (sin desplazar).
    /// Las celdas van en orden de `y`, asi que se busca por biseccion.
    pub fn visibles(&self, y0: f32, y1: f32) -> std::ops::Range<usize> {
        let desde = self.celdas.partition_point(|c| c.y + c.alto < y0);
        let hasta = self.celdas.partition_point(|c| c.y <= y1);
        desde.min(hasta)..hasta
    }

    /// Cuanto hay que desplazar para que se vea entera la celda `i`.
    pub fn scroll_para_ver(&self, i: usize, scroll: f32, alto_vista: f32, escala: f32) -> f32 {
        let Some(c) = self.celdas.get(i) else {
            return scroll;
        };
        // Arriba se deja ver tambien el titulo del dia si es la primera fila.
        let arriba = c.y - TITULO_DIA * escala;
        let abajo = c.y + c.alto + PIE * escala;
        if arriba < scroll {
            arriba.max(0.0)
        } else if abajo > scroll + alto_vista {
            abajo - alto_vista
        } else {
            scroll
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flecha {
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
}

/// Adonde va el foco desde `i` con una flecha. Izquierda y derecha siguen
/// el orden (pasan de un dia a otro); arriba y abajo van a la fila de
/// encima o de debajo, a la celda mas cercana en horizontal.
pub fn mover(d: &Disposicion, i: usize, f: Flecha) -> usize {
    let n = d.celdas.len();
    if n == 0 {
        return 0;
    }
    let i = i.min(n - 1);
    match f {
        Flecha::Izquierda => i.saturating_sub(1),
        Flecha::Derecha => (i + 1).min(n - 1),
        Flecha::Arriba | Flecha::Abajo => {
            let c = d.celdas[i];
            let fila_y = |y: f32| -> Option<f32> {
                let candidatas = d.celdas.iter().map(|o| o.y);
                if f == Flecha::Arriba {
                    candidatas
                        .filter(|&oy| oy < y - 0.5)
                        .fold(None, |m: Option<f32>, oy| Some(m.map_or(oy, |m| m.max(oy))))
                } else {
                    candidatas
                        .filter(|&oy| oy > y + 0.5)
                        .fold(None, |m: Option<f32>, oy| Some(m.map_or(oy, |m| m.min(oy))))
                }
            };
            let Some(y) = fila_y(c.y) else {
                return i;
            };
            d.celdas
                .iter()
                .enumerate()
                .filter(|(_, o)| (o.y - y).abs() < 0.5)
                .min_by(|(_, a), (_, b)| (a.x - c.x).abs().total_cmp(&(b.x - c.x).abs()))
                .map(|(k, _)| k)
                .unwrap_or(i)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_dia_de_la_semana_empieza_en_lunes() {
        // 1-1-1970, jueves; 5-1-1970, lunes; 11-1-1970, domingo.
        assert_eq!(dia_de_la_semana(0), 3);
        assert_eq!(dia_de_la_semana(4), 0);
        assert_eq!(dia_de_la_semana(10), 6);
        // Caso negativo: antes de 1970 tampoco se sale de 0..7.
        assert_eq!(dia_de_la_semana(-1), 2);
    }

    #[test]
    fn la_chapita_dice_los_dias_que_quedan_y_nada_si_se_conserva() {
        let ahora = 100 * DIA_MS + DIA_MS / 2;
        let en = |d: i64| dias_que_quedan(Some(ahora + d * DIA_MS), ahora);
        assert_eq!(dias_que_quedan(None, ahora), None);
        assert_eq!(en(0), Some(0));
        assert_eq!(en(1), Some(1));
        assert_eq!(en(6), Some(6));
        // Caso negativo: lo ya pasado no da dias negativos.
        assert_eq!(en(-2), Some(0));
    }

    #[test]
    fn la_busqueda_no_mira_tildes_ni_mayusculas_y_pide_todas_las_palabras() {
        let campos = [
            plegar("captura-0012.png"),
            plegar("Presupuesto Obra MIRAFLORES · Acero"),
        ];
        let c: Vec<&str> = campos.iter().map(String::as_str).collect();
        assert!(encaja(&plegar("miraflores"), &c));
        assert!(encaja(&plegar("acéro 0012"), &c));
        assert!(encaja("", &c), "sin consulta, todo");
        // Caso negativo: una palabra que no esta basta para que no encaje.
        assert!(!encaja(&plegar("acero ladrillo"), &c));
    }

    #[test]
    fn los_grupos_por_dia_empiezan_fila_nueva() {
        // Cuatro de hoy y dos de ayer, en tres columnas.
        let d = disponer(&[5, 5, 5, 5, 4, 4], 684.0, 1.0);
        assert_eq!(d.columnas, 3);
        assert_eq!(d.grupos.len(), 2);
        assert_eq!((d.grupos[0].desde, d.grupos[0].hasta), (0, 4));
        assert_eq!(
            d.celdas[3].x, d.celdas[0].x,
            "la cuarta abre la segunda fila"
        );
        assert!(d.celdas[3].y > d.celdas[0].y);
        // La primera de ayer, en fila nueva y debajo del titulo de su dia.
        assert_eq!(d.celdas[4].x, d.celdas[0].x);
        assert!(d.celdas[4].y > d.grupos[1].y + TITULO_DIA - 1.0);
        assert!(d.grupos[1].y > d.celdas[3].y + d.celdas[3].alto);
        // Caso negativo: estrecha, nunca cero columnas; vacia, sin grupos.
        assert_eq!(disponer(&[1], 50.0, 1.0).columnas, 1);
        assert!(disponer(&[], 684.0, 1.0).grupos.is_empty());
    }

    #[test]
    fn solo_son_visibles_las_que_asoman() {
        let dias: Vec<i64> = vec![1; 300];
        let d = disponer(&dias, 684.0, 1.0);
        let v = d.visibles(0.0, 600.0);
        assert_eq!(v.start, 0);
        assert!(v.len() <= 15, "unas pocas filas, no 300: {v:?}");
        let fondo = d.visibles(d.alto_total - 600.0, d.alto_total);
        assert_eq!(fondo.end, 300);
        // Caso negativo: muy por debajo de todo, nada.
        assert!(
            d.visibles(d.alto_total + 10.0, d.alto_total + 500.0)
                .is_empty()
        );
    }

    #[test]
    fn las_flechas_mueven_el_foco_y_no_se_salen() {
        // Hoy 4 (dos filas: 3 + 1), ayer 3.
        let d = disponer(&[5, 5, 5, 5, 4, 4, 4], 684.0, 1.0);
        assert_eq!(mover(&d, 0, Flecha::Derecha), 1);
        assert_eq!(
            mover(&d, 1, Flecha::Abajo),
            3,
            "a la fila de abajo, la mas cercana"
        );
        assert_eq!(
            mover(&d, 3, Flecha::Abajo),
            4,
            "de la ultima de hoy a la primera de ayer"
        );
        assert_eq!(mover(&d, 5, Flecha::Arriba), 3);
        assert_eq!(mover(&d, 3, Flecha::Derecha), 4, "derecha sigue el orden");
        // Casos negativos: en los bordes se queda donde esta.
        assert_eq!(mover(&d, 0, Flecha::Izquierda), 0);
        assert_eq!(mover(&d, 1, Flecha::Arriba), 1);
        assert_eq!(mover(&d, 6, Flecha::Abajo), 6);
        assert_eq!(mover(&d, 6, Flecha::Derecha), 6);
    }

    #[test]
    fn el_scroll_trae_la_celda_a_la_vista() {
        let d = disponer(&vec![1; 60], 684.0, 1.0);
        let lejos = 40;
        let s = d.scroll_para_ver(lejos, 0.0, 600.0, 1.0);
        assert!(s > 0.0);
        let c = d.celdas[lejos];
        assert!(c.y >= s && c.y + c.alto <= s + 600.0);
        // Caso negativo: si ya se ve, no se mueve.
        assert_eq!(d.scroll_para_ver(0, 0.0, 600.0, 1.0), 0.0);
    }
}
