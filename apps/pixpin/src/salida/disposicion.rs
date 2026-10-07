//! **Donde cae cada cosa en la Salida**, sin pintar: la cabecera, las
//! tarjetas de los ficheros, la franja de «arrastrar todos» y los botones de
//! abajo. Aparte para probarlo sin GPU: lo que se pinta y lo que se pulsa
//! salen de la MISMA cuenta.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use pixpin_geom::Rect;
use pixpin_render::RectF;

use crate::v2::geom::dentro;

/// Lo ancho de la ventana, en logicos: cabe el nombre de un PDF largo y los
/// cinco botones de abajo, y no tapa media pantalla.
pub const ANCHO: f32 = 380.0;
/// La cabecera: el titulo, la pista de que se arrastra y la ✕.
pub const CABECERA: f32 = 60.0;
/// Una tarjeta: la miniatura de 48 con aire y dos lineas de texto.
pub const ALTO_TARJETA: f32 = 64.0;
/// La franja de «arrastrar los N juntos», solo con mas de uno.
pub const ALTO_TODOS: f32 = 40.0;
/// El pie: los botones de 40 con 8 de aire arriba y abajo.
pub const PIE: f32 = 56.0;
pub const MARGEN: f32 = 12.0;
pub const HUECO: f32 = 8.0;
/// Lo ancho del boton principal (Compartir), con icono y rotulo.
pub const ANCHO_PRINCIPAL: f32 = 132.0;
/// Las tarjetas que caben a la vez: con mas, la ventana ya no es «una
/// ventanita» y tapa lo que el usuario esta haciendo. Las mas viejas se van.
pub const MAX_TARJETAS: usize = 5;
/// Lo que se mueve el raton (en logicos) antes de que pulsar una tarjeta
/// pase a ser arrastrarla. Con menos, un clic con pulso tembloroso
/// arrancaria un arrastre que el usuario no pidio.
pub const UMBRAL_ARRASTRE: f32 = 5.0;
/// El aire con el borde de la pantalla.
pub const AIRE_PANTALLA: u32 = 20;

/// Lo que hacen los botones de abajo, sobre la tarjeta elegida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    Compartir,
    Copiar,
    Abrir,
    Carpeta,
    Pinear,
}

/// Todos, en el orden en que se colocan de izquierda a derecha.
pub const BOTONES: [Boton; 5] = [
    Boton::Compartir,
    Boton::Copiar,
    Boton::Abrir,
    Boton::Carpeta,
    Boton::Pinear,
];

/// Lo que hay bajo un punto de la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Zona {
    #[default]
    Nada,
    /// La cabecera fuera de la ✕: agarrarla mueve la ventana.
    Cabecera,
    Cerrar,
    /// La franja de arrastrar todos juntos.
    Todos,
    Tarjeta(usize),
    Boton(Boton),
}

/// Donde cae cada cosa, en pixeles de la ventana.
#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub ancho: f32,
    pub alto: f32,
    pub cabecera: RectF,
    pub cerrar: RectF,
    pub todos: Option<RectF>,
    pub tarjetas: Vec<RectF>,
    pub botones: Vec<(Boton, RectF)>,
}

/// **Coloca la Salida** para `n` ficheros a escala `s`.
pub fn disponer(n: usize, s: f32) -> Disposicion {
    let n = n.min(MAX_TARJETAS);
    let ancho = ANCHO * s;
    let cabecera = RectF {
        x: 0.0,
        y: 0.0,
        ancho,
        alto: CABECERA * s,
    };
    let lado = crate::v2::OBJETIVO_MINIMO * s;
    let cerrar = RectF {
        x: ancho - (MARGEN - 2.0) * s - lado,
        y: (CABECERA * s - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    let mut y = CABECERA * s;
    let dentro_x = MARGEN * s;
    let dentro_ancho = ancho - 2.0 * MARGEN * s;
    let todos = (n > 1).then(|| {
        let r = RectF {
            x: dentro_x,
            y,
            ancho: dentro_ancho,
            alto: ALTO_TODOS * s,
        };
        y += (ALTO_TODOS + HUECO) * s;
        r
    });
    let mut tarjetas = Vec::with_capacity(n);
    for _ in 0..n {
        tarjetas.push(RectF {
            x: dentro_x,
            y,
            ancho: dentro_ancho,
            alto: ALTO_TARJETA * s,
        });
        y += (ALTO_TARJETA + HUECO) * s;
    }
    // El pie: Compartir (la principal, azul) a la izquierda y los de icono
    // pegados a la derecha. Como la hoja de compartir del movil: la accion
    // principal siempre en el mismo sitio.
    let pie_y = y - HUECO * s;
    let by = pie_y + (PIE - crate::v2::BOTON) / 2.0 * s;
    let alto_boton = crate::v2::BOTON * s;
    let mut botones = vec![(
        Boton::Compartir,
        RectF {
            x: dentro_x,
            y: by,
            ancho: ANCHO_PRINCIPAL * s,
            alto: alto_boton,
        },
    )];
    let iconos = &BOTONES[1..];
    let mut bx = ancho - MARGEN * s - iconos.len() as f32 * (lado + HUECO * s) + HUECO * s;
    for b in iconos {
        botones.push((
            *b,
            RectF {
                x: bx,
                y: by,
                ancho: lado,
                alto: alto_boton,
            },
        ));
        bx += lado + HUECO * s;
    }
    let alto = pie_y + PIE * s;
    Disposicion {
        ancho,
        alto,
        cabecera,
        cerrar,
        todos,
        tarjetas,
        botones,
    }
}

/// **Que hay bajo `p`.** Lo pequeno antes que lo grande: la ✕ esta dentro
/// de la cabecera.
pub fn zona_en(d: &Disposicion, p: (f32, f32)) -> Zona {
    if dentro(d.cerrar, p) {
        return Zona::Cerrar;
    }
    if dentro(d.cabecera, p) {
        return Zona::Cabecera;
    }
    if d.todos.is_some_and(|r| dentro(r, p)) {
        return Zona::Todos;
    }
    if let Some(i) = d.tarjetas.iter().position(|r| dentro(*r, p)) {
        return Zona::Tarjeta(i);
    }
    if let Some((b, _)) = d.botones.iter().find(|(_, r)| dentro(*r, p)) {
        return Zona::Boton(*b);
    }
    Zona::Nada
}

/// Si el raton se ha movido lo bastante desde `desde` para que pulsar pase
/// a ser arrastrar. En pixeles de pantalla, con la escala del monitor.
pub fn supera_umbral(desde: (i32, i32), ahora: (i32, i32), s: f32) -> bool {
    let (dx, dy) = ((ahora.0 - desde.0) as f32, (ahora.1 - desde.1) as f32);
    dx.abs() > UMBRAL_ARRASTRE * s || dy.abs() > UMBRAL_ARRASTRE * s
}

/// **Junta lo que llega con lo que habia**: lo nuevo arriba (es lo que se
/// acaba de hacer y lo que se busca), sin repetir un fichero que ya estaba y
/// sin pasar de [`MAX_TARJETAS`] (se van los mas viejos). Devuelve si cambio
/// algo.
pub fn juntar(actuales: &mut Vec<PathBuf>, nuevos: Vec<PathBuf>) -> bool {
    let antes = actuales.clone();
    let mut todo: Vec<PathBuf> = Vec::with_capacity(actuales.len() + nuevos.len());
    for r in nuevos.into_iter().chain(actuales.drain(..)) {
        if !todo.contains(&r) {
            todo.push(r);
        }
    }
    todo.truncate(MAX_TARJETAS);
    *actuales = todo;
    *actuales != antes
}

/// Abajo a la derecha del area de trabajo del monitor del raton, donde no
/// tapa lo que se estaba haciendo (y donde Windows saca sus avisos).
pub fn marco_inicial(area: Rect, ancho: u32, alto: u32, escala_por_cien: u32) -> Rect {
    let aire = AIRE_PANTALLA * escala_por_cien / 100;
    let (w, h) = (ancho.min(area.ancho), alto.min(area.alto));
    Rect {
        x: area.x + area.ancho as i32 - w as i32 - aire as i32,
        y: area.y + area.alto as i32 - h as i32 - aire as i32,
        ancho: w,
        alto: h,
    }
}

/// El mismo marco con otro alto, creciendo o encogiendo HACIA ARRIBA si la
/// ventana estaba pegada abajo (donde nace): una tarjeta mas no puede
/// empujar la ventana fuera de la pantalla.
pub fn con_alto(marco: Rect, alto: u32, area: Rect) -> Rect {
    let abajo = marco.y + marco.alto as i32;
    let mut y = abajo - alto as i32;
    if y < area.y {
        y = area.y;
    }
    Rect { y, alto, ..marco }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn centro(r: RectF) -> (f32, f32) {
        (r.x + r.ancho / 2.0, r.y + r.alto / 2.0)
    }

    #[test]
    fn cada_tarjeta_y_cada_boton_se_encuentra_donde_se_pinto() {
        let d = disponer(3, 1.5);
        assert_eq!(zona_en(&d, centro(d.cerrar)), Zona::Cerrar);
        assert_eq!(zona_en(&d, (20.0, 10.0)), Zona::Cabecera);
        assert_eq!(zona_en(&d, centro(d.todos.unwrap())), Zona::Todos);
        for (i, t) in d.tarjetas.iter().enumerate() {
            assert_eq!(zona_en(&d, centro(*t)), Zona::Tarjeta(i));
        }
        for (b, r) in &d.botones {
            assert_eq!(zona_en(&d, centro(*r)), Zona::Boton(*b));
        }
        // Todo cabe dentro de la ventana, y los botones no se pisan.
        for (_, r) in &d.botones {
            assert!(r.x >= 0.0 && r.x + r.ancho <= d.ancho + 0.01);
            assert!(r.y + r.alto <= d.alto + 0.01);
        }
        for w in d.botones.windows(2) {
            assert!(w[0].1.x + w[0].1.ancho <= w[1].1.x);
        }
        // Caso negativo: el hueco entre dos tarjetas no es ninguna.
        let entre = (
            d.tarjetas[0].x + 10.0,
            d.tarjetas[0].y + d.tarjetas[0].alto + 2.0,
        );
        assert_eq!(zona_en(&d, entre), Zona::Nada);
    }

    #[test]
    fn con_un_solo_fichero_no_hay_franja_de_arrastrar_todos() {
        let d = disponer(1, 1.0);
        assert!(d.todos.is_none());
        assert_eq!(d.tarjetas.len(), 1);
        assert_eq!(d.tarjetas[0].y, CABECERA);
        // Caso negativo: con dos ya esta, y empuja las tarjetas hacia abajo.
        let d2 = disponer(2, 1.0);
        assert!(d2.todos.is_some());
        assert!(d2.tarjetas[0].y > CABECERA + ALTO_TODOS);
    }

    #[test]
    fn la_ventana_crece_con_las_tarjetas_hasta_el_tope() {
        let uno = disponer(1, 1.0).alto;
        let dos = disponer(2, 1.0).alto;
        assert!(dos > uno);
        assert_eq!(disponer(MAX_TARJETAS, 1.0), disponer(MAX_TARJETAS + 7, 1.0));
        // Los objetivos miden al menos 40 (principio v2).
        let d = disponer(1, 1.0);
        assert!(d.botones.iter().all(|(_, r)| r.ancho >= 40.0 && r.alto >= 40.0));
        assert!(d.cerrar.ancho >= 40.0);
    }

    #[test]
    fn pulsar_y_temblar_no_es_arrastrar() {
        assert!(!supera_umbral((100, 100), (103, 98), 1.0));
        assert!(supera_umbral((100, 100), (107, 100), 1.0));
        assert!(supera_umbral((100, 100), (100, 93), 1.0));
        // Caso negativo: con la pantalla al 200 % el mismo temblor en
        // pixeles fisicos (8) aun no arrastra.
        assert!(!supera_umbral((100, 100), (108, 100), 2.0));
    }

    #[test]
    fn lo_nuevo_va_arriba_sin_repetir_y_sin_pasar_del_tope() {
        let p = |s: &str| PathBuf::from(format!(r"C:\x\{s}"));
        let mut v = vec![p("a"), p("b")];
        assert!(juntar(&mut v, vec![p("c"), p("a")]));
        assert_eq!(v, vec![p("c"), p("a"), p("b")]);
        let mas: Vec<PathBuf> = (0..9).map(|i| p(&format!("n{i}"))).collect();
        juntar(&mut v, mas);
        assert_eq!(v.len(), MAX_TARJETAS);
        assert_eq!(v[0], p("n0"));
        // Caso negativo: volver a mandar lo mismo, en el mismo orden, no
        // cambia nada (y no hace falta repintar).
        let mut w = vec![p("a"), p("b")];
        assert!(!juntar(&mut w, vec![p("a")]));
        assert_eq!(w, vec![p("a"), p("b")]);
    }

    #[test]
    fn nace_abajo_a_la_derecha_y_crece_hacia_arriba() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let m = marco_inicial(area, 380, 200, 100);
        assert_eq!((m.x, m.y), (1920 - 380 - 20, 1040 - 200 - 20));
        let mas = con_alto(m, 300, area);
        assert_eq!(mas.y + mas.alto as i32, m.y + m.alto as i32, "el borde de abajo se queda");
        // Caso negativo: no sube por encima de la pantalla.
        let enorme = con_alto(m, 5000, area);
        assert_eq!(enorme.y, 0);
        // En un monitor a la izquierda (x negativa) tambien cae dentro.
        let izq = Rect {
            x: -1920,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let m2 = marco_inicial(izq, 570, 300, 150);
        assert_eq!(m2.x, -570 - 30);
    }
}
