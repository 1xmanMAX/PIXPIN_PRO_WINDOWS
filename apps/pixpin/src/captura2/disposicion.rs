//! **Donde cae cada cosa** de la captura v2, sin pantalla.
//!
//! Todo en pixeles fisicos del escritorio virtual (los mismos que trae el
//! raton), con las medidas de la maqueta en pixeles logicos multiplicadas
//! por la escala del monitor. Pintar y repartir el clic salen de aqui: si
//! salieran de dos sitios, un dia el boton responderia donde no se ve.

use pixpin_geom::{Punto, Rect};

/// Lo que se elige en la barra de abajo. Cada uno con su letra (D: solo
/// valen dentro del overlay enfocado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Arrastrar una zona libre, o un clic sobre lo resaltado.
    Zona,
    /// La ventana entera bajo el raton.
    Ventana,
    /// La pantalla entera bajo el raton.
    Pantalla,
    Scroll,
    Gif,
    PinEnVivo,
    /// Leer el texto de la zona (OCR) y copiarlo.
    Texto,
}

impl Modo {
    pub const TODOS: [Modo; 7] = [
        Modo::Zona,
        Modo::Ventana,
        Modo::Pantalla,
        Modo::Scroll,
        Modo::Gif,
        Modo::PinEnVivo,
        Modo::Texto,
    ];

    pub fn letra(self) -> char {
        match self {
            Modo::Zona => 'Z',
            Modo::Ventana => 'V',
            Modo::Pantalla => 'P',
            Modo::Scroll => 'S',
            Modo::Gif => 'G',
            Modo::PinEnVivo => 'L',
            Modo::Texto => 'T',
        }
    }

    /// El modo de una tecla virtual (las letras van en mayuscula).
    pub fn de_tecla(vk: u32) -> Option<Modo> {
        let c = char::from_u32(vk)?;
        Modo::TODOS.into_iter().find(|m| m.letra() == c)
    }
}

/// Las proporciones de la barra de abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proporcion {
    Libre,
    P16x9,
    P4x3,
    P1x1,
}

impl Proporcion {
    pub const TODAS: [Proporcion; 4] = [
        Proporcion::Libre,
        Proporcion::P16x9,
        Proporcion::P4x3,
        Proporcion::P1x1,
    ];

    pub fn par(self) -> Option<(u32, u32)> {
        match self {
            Proporcion::Libre => None,
            Proporcion::P16x9 => Some((16, 9)),
            Proporcion::P4x3 => Some((4, 3)),
            Proporcion::P1x1 => Some((1, 1)),
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Proporcion::Libre => "",
            Proporcion::P16x9 => "16:9",
            Proporcion::P4x3 => "4:3",
            Proporcion::P1x1 => "1:1",
        }
    }
}

/// Que hay bajo el raton en la barra de los modos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnModos {
    Modo(Modo),
    Ancho,
    Alto,
    Proporcion(Proporcion),
    Repetir,
    /// Dentro del panel pero en ningun boton: el clic se lo traga igual.
    Fondo,
}

/// Medidas de la maqueta (pixeles logicos).
const PADDING: u32 = 8;
const MODO_ANCHO: u32 = 100;
const MODO_ALTO: u32 = 76;
const HUECO: u32 = 4;
const SEPARADOR: u32 = 13;
const FILA2_ALTO: u32 = 40;
const CAMPO_ANCHO: u32 = 112;
const POR_ANCHO: u32 = 20;
const HUECO2: u32 = 8;
const CHIP_ANCHOS: [u32; 4] = [64, 58, 50, 50];
const REPETIR_ANCHO: u32 = 250;
const MARGEN_ABAJO: u32 = 24;

fn e(v: u32, escala: u32) -> u32 {
    v * escala.max(100) / 100
}

/// La barra de los modos, abajo en el centro del monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct BarraModos {
    pub panel: Rect,
    pub modos: Vec<(Modo, Rect)>,
    /// Donde va la raya entre los de capturar y los de hacer otra cosa.
    pub separador_x: i32,
    pub linea_y: i32,
    pub ancho: Rect,
    pub por: Rect,
    pub alto: Rect,
    pub chips: Vec<(Proporcion, Rect)>,
    /// Solo si hay una zona que repetir: sin ella el boton no se pone.
    pub repetir: Option<Rect>,
    pub escala: u32,
}

impl BarraModos {
    pub fn colocar(area_trabajo: Rect, escala: u32, con_repetir: bool) -> BarraModos {
        let s = |v| e(v, escala);
        let fila1 = 7 * s(MODO_ANCHO) + 6 * s(HUECO) + s(SEPARADOR);
        let chips: u32 = CHIP_ANCHOS.iter().map(|c| s(*c)).sum::<u32>() + 4 * s(HUECO2);
        let mut fila2 = s(CAMPO_ANCHO) * 2 + s(POR_ANCHO) + 2 * s(HUECO2) + s(HUECO2) + chips;
        if con_repetir {
            fila2 += s(SEPARADOR) + s(REPETIR_ANCHO);
        }
        let interior = fila1.max(fila2);
        let ancho = interior + 2 * s(PADDING);
        let alto = s(PADDING) * 4 + s(MODO_ALTO) + 1 + s(FILA2_ALTO);
        let x = area_trabajo.x + (area_trabajo.ancho as i32 - ancho as i32) / 2;
        let y = area_trabajo.abajo() - s(MARGEN_ABAJO) as i32 - alto as i32;
        let panel = Rect { x, y, ancho, alto };

        // Fila 1: los siete, centrados, con la raya tras «Con scroll».
        let mut cx = x + s(PADDING) as i32 + (interior as i32 - fila1 as i32) / 2;
        let fy = y + s(PADDING) as i32;
        let mut modos = Vec::with_capacity(7);
        let mut separador_x = 0;
        for (i, m) in Modo::TODOS.into_iter().enumerate() {
            if i == 4 {
                separador_x = cx - s(HUECO) as i32 + s(SEPARADOR) as i32 / 2;
                cx += s(SEPARADOR) as i32;
            }
            modos.push((
                m,
                Rect {
                    x: cx,
                    y: fy,
                    ancho: s(MODO_ANCHO),
                    alto: s(MODO_ALTO),
                },
            ));
            cx += (s(MODO_ANCHO) + s(HUECO)) as i32;
        }
        let linea_y = fy + s(MODO_ALTO) as i32 + s(PADDING) as i32;

        // Fila 2: medidas, proporciones y repetir.
        let y2 = linea_y + 1 + s(PADDING) as i32;
        let mut x2 = x + s(PADDING) as i32 + (interior as i32 - fila2 as i32) / 2;
        let caja = |x: i32, ancho: u32| Rect {
            x,
            y: y2,
            ancho,
            alto: s(FILA2_ALTO),
        };
        let r_ancho = caja(x2, s(CAMPO_ANCHO));
        x2 += (s(CAMPO_ANCHO) + s(HUECO2)) as i32;
        let r_por = caja(x2, s(POR_ANCHO));
        x2 += (s(POR_ANCHO) + s(HUECO2)) as i32;
        let r_alto = caja(x2, s(CAMPO_ANCHO));
        x2 += (s(CAMPO_ANCHO) + s(HUECO2)) as i32;
        let mut chips_v = Vec::with_capacity(4);
        for (p, w) in Proporcion::TODAS.into_iter().zip(CHIP_ANCHOS) {
            chips_v.push((p, caja(x2, s(w))));
            x2 += (s(w) + s(HUECO2)) as i32;
        }
        let repetir = con_repetir.then(|| {
            x2 += s(SEPARADOR) as i32 - s(HUECO2) as i32;
            caja(x2, s(REPETIR_ANCHO))
        });
        BarraModos {
            panel,
            modos,
            separador_x,
            linea_y,
            ancho: r_ancho,
            por: r_por,
            alto: r_alto,
            chips: chips_v,
            repetir,
            escala,
        }
    }

    pub fn en(&self, p: Punto) -> Option<EnModos> {
        if !self.panel.contiene(p) {
            return None;
        }
        if let Some((m, _)) = self.modos.iter().find(|(_, r)| r.contiene(p)) {
            return Some(EnModos::Modo(*m));
        }
        if self.ancho.contiene(p) {
            return Some(EnModos::Ancho);
        }
        if self.alto.contiene(p) {
            return Some(EnModos::Alto);
        }
        if let Some((c, _)) = self.chips.iter().find(|(_, r)| r.contiene(p)) {
            return Some(EnModos::Proporcion(*c));
        }
        if self.repetir.is_some_and(|r| r.contiene(p)) {
            return Some(EnModos::Repetir);
        }
        Some(EnModos::Fondo)
    }
}

/// La fila de pistas de arriba (los gestos con Alt). Solo informa: no
/// tiene botones, y el clic pasa a la seleccion.
pub fn fila_de_pistas(area_trabajo: Rect, escala: u32) -> Rect {
    let s = |v| e(v, escala);
    Rect {
        x: area_trabajo.x + s(24) as i32,
        y: area_trabajo.y + s(20) as i32,
        ancho: area_trabajo.ancho.saturating_sub(2 * s(24)),
        alto: s(48),
    }
}

/// Las acciones de despues de elegir, en su orden: la principal primero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Copiar,
    Pinear,
    AlChat,
    Texto,
    Guardar,
    Descartar,
}

impl Accion {
    pub const TODAS: [Accion; 6] = [
        Accion::Copiar,
        Accion::Pinear,
        Accion::AlChat,
        Accion::Texto,
        Accion::Guardar,
        Accion::Descartar,
    ];

    /// La chapita del atajo, tal cual se ensena.
    pub fn atajo(self) -> &'static str {
        match self {
            Accion::Copiar => "Enter",
            Accion::Pinear => "Ctrl P",
            Accion::AlChat => "Ctrl M",
            Accion::Texto => "Ctrl T",
            Accion::Guardar => "Ctrl S",
            Accion::Descartar => "Esc",
        }
    }

    fn ancho_logico(self) -> u32 {
        match self {
            Accion::Copiar => 136,
            Accion::Pinear => 140,
            Accion::AlChat => 180,
            Accion::Texto => 132,
            Accion::Guardar => 144,
            Accion::Descartar => 136,
        }
    }

    /// La accion de una tecla, con Ctrl o sin el. Enter y Ctrl+C copian.
    pub fn de_tecla(vk: u32, ctrl: bool, shift: bool) -> Option<Accion> {
        const VK_RETURN: u32 = 0x0D;
        const VK_ESCAPE: u32 = 0x1B;
        match (vk, ctrl, shift) {
            (VK_RETURN, false, _) => Some(Accion::Copiar),
            (VK_ESCAPE, _, _) => Some(Accion::Descartar),
            (v, true, false) if v == u32::from(b'C') => Some(Accion::Copiar),
            (v, true, false) if v == u32::from(b'P') => Some(Accion::Pinear),
            (v, true, false) if v == u32::from(b'M') => Some(Accion::AlChat),
            (v, true, false) if v == u32::from(b'T') => Some(Accion::Texto),
            (v, true, false) if v == u32::from(b'S') => Some(Accion::Guardar),
            _ => None,
        }
    }
}

/// Las herramientas de anotar, con su numero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Util {
    Lapiz,
    Flecha,
    Rectangulo,
    Texto,
    Mosaico,
}

impl Util {
    pub const TODAS: [Util; 5] = [
        Util::Lapiz,
        Util::Flecha,
        Util::Rectangulo,
        Util::Texto,
        Util::Mosaico,
    ];

    pub fn numero(self) -> u32 {
        Util::TODAS.iter().position(|u| *u == self).unwrap_or(0) as u32 + 1
    }

    /// `1`..`5`, de la fila de arriba o del teclado numerico.
    pub fn de_tecla(vk: u32) -> Option<Util> {
        let n = match vk {
            0x31..=0x35 => vk - 0x30,
            0x61..=0x65 => vk - 0x60,
            _ => return None,
        };
        Util::TODAS.get(n as usize - 1).copied()
    }
}

/// Los cinco colores de la maqueta: rojo, amarillo, verde, azul y negro.
pub const COLORES: [[u8; 3]; 5] = [
    [0xFF, 0x3B, 0x30],
    [0xFF, 0xD6, 0x0A],
    [0x30, 0xD1, 0x58],
    [0x0A, 0x84, 0xFF],
    [0x1C, 0x1C, 0x1E],
];

/// Que hay bajo el raton en la barra de anotar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnAnotar {
    Util(Util),
    Color(usize),
    Grosor(usize),
    Deshacer,
    Rehacer,
    Fondo,
}

const ANOTAR_ALTO: u32 = 52;
const BOTON: u32 = 44;
const MUESTRA: u32 = 28;
const TAMANO_ANCHO: u32 = 104;
const CHAPA_ANCHO: u32 = 60;

/// La barra de anotar, pegada encima de la zona.
#[derive(Debug, Clone, PartialEq)]
pub struct BarraAnotar {
    pub panel: Rect,
    pub tamano: Rect,
    pub utiles: Vec<(Util, Rect)>,
    pub colores: Vec<Rect>,
    pub grosores: Vec<Rect>,
    pub deshacer: Rect,
    pub rehacer: Rect,
    pub chapa: Rect,
    /// Las rayas entre grupos, en x.
    pub separadores: Vec<i32>,
}

impl BarraAnotar {
    pub fn ancho(escala: u32) -> u32 {
        let s = |v| e(v, escala);
        s(4) + s(TAMANO_ANCHO)
            + 4 * s(SEPARADOR)
            + 5 * s(BOTON)
            + 4 * s(2)
            + 2 * s(6)
            + 5 * s(MUESTRA)
            + 4 * s(6)
            + 3 * s(BOTON)
            + 2 * s(2)
            + 2 * s(BOTON)
            + s(2)
            + s(CHAPA_ANCHO)
            + s(8)
    }

    pub fn alto(escala: u32) -> u32 {
        e(ANOTAR_ALTO, escala)
    }

    pub fn en_origen(x: i32, y: i32, escala: u32) -> BarraAnotar {
        let s = |v| e(v, escala);
        let alto = s(ANOTAR_ALTO);
        let panel = Rect {
            x,
            y,
            ancho: Self::ancho(escala),
            alto,
        };
        let centro = |h: u32| y + (alto as i32 - h as i32) / 2;
        let boton = |x: i32| Rect {
            x,
            y: centro(s(BOTON)),
            ancho: s(BOTON),
            alto: s(BOTON),
        };
        let mut cx = x + s(4) as i32;
        let tamano = Rect {
            x: cx,
            y: centro(s(BOTON)),
            ancho: s(TAMANO_ANCHO),
            alto: s(BOTON),
        };
        cx += s(TAMANO_ANCHO) as i32;
        let mut separadores = Vec::new();
        let mut sep = |cx: &mut i32| {
            separadores.push(*cx + s(SEPARADOR) as i32 / 2);
            *cx += s(SEPARADOR) as i32;
        };
        sep(&mut cx);
        let mut utiles = Vec::new();
        for (i, u) in Util::TODAS.into_iter().enumerate() {
            if i > 0 {
                cx += s(2) as i32;
            }
            utiles.push((u, boton(cx)));
            cx += s(BOTON) as i32;
        }
        sep(&mut cx);
        cx += s(6) as i32;
        let mut colores = Vec::new();
        for i in 0..COLORES.len() {
            if i > 0 {
                cx += s(6) as i32;
            }
            colores.push(Rect {
                x: cx,
                y: centro(s(MUESTRA)),
                ancho: s(MUESTRA),
                alto: s(MUESTRA),
            });
            cx += s(MUESTRA) as i32;
        }
        cx += s(6) as i32;
        sep(&mut cx);
        let mut grosores = Vec::new();
        for i in 0..3 {
            if i > 0 {
                cx += s(2) as i32;
            }
            grosores.push(boton(cx));
            cx += s(BOTON) as i32;
        }
        sep(&mut cx);
        let deshacer = boton(cx);
        cx += (s(BOTON) + s(2)) as i32;
        let rehacer = boton(cx);
        cx += (s(BOTON) + s(2)) as i32;
        let chapa = Rect {
            x: cx,
            y: centro(s(24)),
            ancho: s(CHAPA_ANCHO),
            alto: s(24),
        };
        BarraAnotar {
            panel,
            tamano,
            utiles,
            colores,
            grosores,
            deshacer,
            rehacer,
            chapa,
            separadores,
        }
    }

    pub fn en(&self, p: Punto) -> Option<EnAnotar> {
        if !self.panel.contiene(p) {
            return None;
        }
        if let Some((u, _)) = self.utiles.iter().find(|(_, r)| r.contiene(p)) {
            return Some(EnAnotar::Util(*u));
        }
        if let Some(i) = self.colores.iter().position(|r| r.contiene(p)) {
            return Some(EnAnotar::Color(i));
        }
        if let Some(i) = self.grosores.iter().position(|r| r.contiene(p)) {
            return Some(EnAnotar::Grosor(i));
        }
        if self.deshacer.contiene(p) {
            return Some(EnAnotar::Deshacer);
        }
        if self.rehacer.contiene(p) {
            return Some(EnAnotar::Rehacer);
        }
        Some(EnAnotar::Fondo)
    }
}

const ACCIONES_ALTO: u32 = 56;

/// La barra de acciones, pegada bajo la zona.
#[derive(Debug, Clone, PartialEq)]
pub struct BarraAcciones {
    pub panel: Rect,
    pub botones: Vec<(Accion, Rect)>,
    pub separador_x: i32,
}

impl BarraAcciones {
    pub fn ancho(escala: u32) -> u32 {
        let s = |v| e(v, escala);
        Accion::TODAS.iter().map(|a| s(a.ancho_logico())).sum::<u32>()
            + 6 * s(6)
            + s(SEPARADOR)
    }

    pub fn alto(escala: u32) -> u32 {
        e(ACCIONES_ALTO, escala)
    }

    pub fn en_origen(x: i32, y: i32, escala: u32) -> BarraAcciones {
        let s = |v| e(v, escala);
        let panel = Rect {
            x,
            y,
            ancho: Self::ancho(escala),
            alto: s(ACCIONES_ALTO),
        };
        let mut cx = x + s(6) as i32;
        let mut botones = Vec::new();
        let mut separador_x = 0;
        for a in Accion::TODAS {
            if a == Accion::Descartar {
                separador_x = cx - s(6) as i32 + s(SEPARADOR) as i32 / 2;
                cx += s(SEPARADOR) as i32 - s(6) as i32;
            }
            botones.push((
                a,
                Rect {
                    x: cx,
                    y: y + s(6) as i32,
                    ancho: s(a.ancho_logico()),
                    alto: s(44),
                },
            ));
            cx += (s(a.ancho_logico()) + s(6)) as i32;
        }
        BarraAcciones {
            panel,
            botones,
            separador_x,
        }
    }

    pub fn en(&self, p: Punto) -> Option<Option<Accion>> {
        if !self.panel.contiene(p) {
            return None;
        }
        Some(self.botones.iter().find(|(_, r)| r.contiene(p)).map(|(a, _)| *a))
    }

    pub fn boton(&self, a: Accion) -> Option<Rect> {
        self.botones.iter().find(|(b, _)| *b == a).map(|(_, r)| *r)
    }
}

/// Donde van las dos barras de despues, alrededor de la zona elegida.
///
/// Lo preferido es la de anotar encima y la de acciones debajo (la maqueta).
/// Si arriba no cabe, la de anotar baja detras de la de acciones; si abajo
/// tampoco, las dos entran en la zona, pegadas a sus bordes. Siempre dentro
/// del area de trabajo del monitor.
pub fn colocar_despues(sel: Rect, area: Rect, escala: u32) -> (BarraAnotar, BarraAcciones) {
    let s = |v| e(v, escala);
    let (wa, ha) = (BarraAnotar::ancho(escala), BarraAnotar::alto(escala));
    let (wb, hb) = (BarraAcciones::ancho(escala), BarraAcciones::alto(escala));
    let x_de = |w: u32| {
        let min = area.x + s(8) as i32;
        let max = (area.derecha() - w as i32 - s(8) as i32).max(min);
        sel.x.clamp(min, max)
    };
    let hueco = s(10) as i32;
    let cabe_abajo = |y: i32, h: u32| y + h as i32 <= area.abajo() - s(8) as i32;

    let y_b_abajo = sel.abajo() + hueco;
    let acciones_fuera = cabe_abajo(y_b_abajo, hb);
    let y_b = if acciones_fuera {
        y_b_abajo
    } else {
        sel.abajo() - hueco - hb as i32
    };
    let y_a_arriba = sel.y - hueco - ha as i32;
    let y_a = if y_a_arriba >= area.y + s(8) as i32 {
        y_a_arriba
    } else if acciones_fuera && cabe_abajo(y_b + hb as i32 + s(8) as i32, ha) {
        y_b + hb as i32 + s(8) as i32
    } else {
        sel.y + hueco
    };
    (
        BarraAnotar::en_origen(x_de(wa), y_a, escala),
        BarraAcciones::en_origen(x_de(wb), y_b.max(area.y), escala),
    )
}

/// El boton unico de confirmar de los modos que no preguntan (con scroll,
/// GIF, pin en vivo, texto): bajo la zona, o dentro si no cabe.
pub fn boton_confirmar(sel: Rect, area: Rect, escala: u32) -> Rect {
    let s = |v| e(v, escala);
    let (w, h) = (s(220), s(40));
    let min = area.x + s(8) as i32;
    let max = (area.derecha() - w as i32 - s(8) as i32).max(min);
    let x = sel.x.clamp(min, max);
    let y = if sel.abajo() + s(10) as i32 + h as i32 <= area.abajo() - s(8) as i32 {
        sel.abajo() + s(10) as i32
    } else {
        sel.abajo() - s(10) as i32 - h as i32
    };
    Rect { x, y, ancho: w, alto: h }
}

/// Que hay bajo el raton en el selector de proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnSelector {
    Buscar,
    Fila(usize),
    Comentario,
    Enviar,
    Fondo,
}

/// Cuantos proyectos se ensenan a la vez. Los demas, buscando.
pub const FILAS_SELECTOR: usize = 6;

/// El selector de proyecto de «Al chat», colgado del boton.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub panel: Rect,
    pub buscar: Rect,
    pub rotulo_y: i32,
    pub filas: Vec<Rect>,
    pub linea_y: i32,
    pub comentario: Rect,
    pub enviar: Rect,
}

impl Selector {
    pub fn colocar(boton: Rect, area: Rect, filas: usize, escala: u32) -> Selector {
        let s = |v| e(v, escala);
        let n = filas.clamp(1, FILAS_SELECTOR) as u32;
        let ancho = s(340);
        let alto = s(8) + s(40) + s(28) + n * s(44) + s(13) + s(40) + s(8);
        let min = area.x + s(8) as i32;
        let max = (area.derecha() - ancho as i32 - s(8) as i32).max(min);
        let x = boton.x.clamp(min, max);
        let abajo = boton.abajo() + s(14) as i32;
        let y = if abajo + alto as i32 <= area.abajo() - s(8) as i32 {
            abajo
        } else {
            (boton.y - s(14) as i32 - alto as i32).max(area.y + s(8) as i32)
        };
        let panel = Rect { x, y, ancho, alto };
        let ix = x + s(8) as i32;
        let iw = ancho - 2 * s(8);
        let buscar = Rect {
            x: ix,
            y: y + s(8) as i32,
            ancho: iw,
            alto: s(40),
        };
        let rotulo_y = buscar.abajo();
        let mut v = Vec::new();
        let mut fy = rotulo_y + s(28) as i32;
        for _ in 0..filas.min(FILAS_SELECTOR) {
            v.push(Rect {
                x: ix,
                y: fy,
                ancho: iw,
                alto: s(44),
            });
            fy += s(44) as i32;
        }
        // Sin filas que ensenar (la busqueda no encontro nada) el hueco de
        // una fila se queda para el aviso: el panel no da saltos.
        let fin_filas = rotulo_y + s(28) as i32 + (n * s(44)) as i32;
        let linea_y = fin_filas + s(6) as i32;
        let wy = linea_y + s(7) as i32;
        let enviar_w = s(84);
        let comentario = Rect {
            x: ix,
            y: wy,
            ancho: iw - enviar_w - s(8),
            alto: s(40),
        };
        let enviar = Rect {
            x: comentario.derecha() + s(8) as i32,
            y: wy,
            ancho: enviar_w,
            alto: s(40),
        };
        Selector {
            panel,
            buscar,
            rotulo_y,
            filas: v,
            linea_y,
            comentario,
            enviar,
        }
    }

    pub fn en(&self, p: Punto) -> Option<EnSelector> {
        if !self.panel.contiene(p) {
            return None;
        }
        if self.buscar.contiene(p) {
            return Some(EnSelector::Buscar);
        }
        if let Some(i) = self.filas.iter().position(|r| r.contiene(p)) {
            return Some(EnSelector::Fila(i));
        }
        if self.comentario.contiene(p) {
            return Some(EnSelector::Comentario);
        }
        if self.enviar.contiene(p) {
            return Some(EnSelector::Enviar);
        }
        Some(EnSelector::Fondo)
    }
}

/// Los proyectos que casan con lo buscado, en el orden de la lista: sin
/// distinguir mayusculas ni tildes, por cualquier trozo del nombre.
pub fn filtrar<'a>(proyectos: &'a [(String, String)], busqueda: &str) -> Vec<&'a (String, String)> {
    let b = sin_tildes(busqueda.trim());
    proyectos
        .iter()
        .filter(|(_, nombre)| b.is_empty() || sin_tildes(nombre).contains(&b))
        .collect()
}

fn sin_tildes(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otro => otro,
        })
        .collect()
}

const LUPA_ANCHO: u32 = 200;
const LUPA_IMAGEN_ALTO: u32 = 120;
const LUPA_ALTO: u32 = 252;

/// La lupa con el codigo de color, junto al cursor y del lado donde quepa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLupa {
    pub panel: Rect,
    pub imagen: Rect,
    /// Cuantos pixeles reales caben a lo ancho y a lo alto del cristal.
    pub fuente: (u32, u32),
}

/// Aumento de la lupa: cada pixel de la pantalla se ve de ocho.
pub const AUMENTO_LUPA: u32 = 8;

impl PanelLupa {
    pub fn colocar(cursor: Punto, monitor: Rect, escala: u32) -> PanelLupa {
        let s = |v| e(v, escala);
        let (w, h) = (s(LUPA_ANCHO), s(LUPA_ALTO));
        let m = s(24) as i32;
        let mut x = cursor.x + m;
        let mut y = cursor.y + m;
        if x + w as i32 > monitor.derecha() {
            x = cursor.x - m - w as i32;
        }
        if y + h as i32 > monitor.abajo() {
            y = cursor.y - m - h as i32;
        }
        x = x.clamp(monitor.x, (monitor.derecha() - w as i32).max(monitor.x));
        y = y.clamp(monitor.y, (monitor.abajo() - h as i32).max(monitor.y));
        let imagen = Rect {
            x: x + s(10) as i32,
            y: y + s(10) as i32,
            ancho: w - 2 * s(10),
            alto: s(LUPA_IMAGEN_ALTO),
        };
        // Impar, para que el pixel del cursor caiga justo en el centro.
        let impar = |v: u32| {
            let n = (v / (AUMENTO_LUPA * escala.max(100) / 100)).max(1);
            if n % 2 == 0 { n - 1 } else { n }.max(1)
        };
        PanelLupa {
            panel: Rect {
                x,
                y,
                ancho: w,
                alto: h,
            },
            imagen,
            fuente: (impar(imagen.ancho), impar(imagen.alto)),
        }
    }

    /// La region de la pantalla que se amplia, centrada en el cursor y
    /// corrida (no encogida) para no salirse del monitor.
    pub fn region(&self, cursor: Punto, monitor: Rect) -> Rect {
        let (fw, fh) = self.fuente;
        let x = (cursor.x - fw as i32 / 2).clamp(monitor.x, monitor.derecha() - fw as i32);
        let y = (cursor.y - fh as i32 / 2).clamp(monitor.y, monitor.abajo() - fh as i32);
        Rect {
            x,
            y,
            ancho: fw,
            alto: fh,
        }
    }
}

/// La etiqueta con el nombre de la ventana resaltada, encima de ella (o
/// dentro, si arriba no cabe).
pub fn etiqueta_de_ventana(resaltado: Rect, ancho: u32, monitor: Rect, escala: u32) -> Rect {
    let s = |v| e(v, escala);
    let alto = s(38);
    let y = if resaltado.y - (s(12) + alto) as i32 >= monitor.y {
        resaltado.y - (s(12) + alto) as i32
    } else {
        resaltado.y.max(monitor.y) + s(8) as i32
    };
    let min = monitor.x + s(8) as i32;
    let max = (monitor.derecha() - ancho as i32 - s(8) as i32).max(min);
    Rect {
        x: resaltado.x.clamp(min, max),
        y,
        ancho,
        alto,
    }
}

/// Un campo de medida que se esta tecleando: solo cifras, cinco como mucho.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cifras(pub String);

impl Cifras {
    pub fn escribir(&mut self, c: char) -> bool {
        if c.is_ascii_digit() && self.0.len() < 5 {
            self.0.push(c);
            true
        } else {
            false
        }
    }

    pub fn borrar(&mut self) -> bool {
        self.0.pop().is_some()
    }

    pub fn valor(&self) -> Option<u32> {
        self.0.parse::<u32>().ok().filter(|v| *v > 0)
    }
}

/// La zona de `ancho` x `alto` que piden las medidas tecleadas: con
/// seleccion, desde su esquina; sin ella, centrada en el monitor.
pub fn zona_de_medidas(ancho: u32, alto: u32, seleccion: Option<Rect>, monitor: Rect) -> Rect {
    match seleccion.filter(|s| !s.esta_vacio()) {
        Some(s) => Rect {
            x: s.x,
            y: s.y,
            ancho,
            alto,
        },
        None => Rect {
            x: monitor.x + (monitor.ancho as i32 - ancho as i32) / 2,
            y: monitor.y + (monitor.alto as i32 - alto as i32) / 2,
            ancho,
            alto,
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        }
    }

    #[test]
    fn las_letras_de_los_modos_son_las_de_la_maqueta_y_no_se_repiten() {
        let letras: String = Modo::TODOS.iter().map(|m| m.letra()).collect();
        assert_eq!(letras, "ZVPSGLT");
        for m in Modo::TODOS {
            assert_eq!(Modo::de_tecla(m.letra() as u32), Some(m));
        }
        // Caso negativo: R es repetir, C copiar el color; no son modos.
        assert_eq!(Modo::de_tecla(u32::from(b'R')), None);
        assert_eq!(Modo::de_tecla(u32::from(b'C')), None);
        assert_eq!(Modo::de_tecla(0x31), None);
    }

    #[test]
    fn la_barra_de_modos_va_abajo_en_el_centro_y_reparte_el_clic() {
        let b = BarraModos::colocar(area(), 100, true);
        assert!(b.panel.abajo() <= 1040 - 24);
        let centro = b.panel.x + b.panel.ancho as i32 / 2;
        assert!((centro - 960).abs() <= 1);
        for (m, r) in &b.modos {
            let p = Punto {
                x: r.x + 10,
                y: r.y + 10,
            };
            assert_eq!(b.en(p), Some(EnModos::Modo(*m)));
            assert!(r.ancho >= 40 && r.alto >= 40, "objetivos de 40 px o mas");
        }
        let r = b.repetir.unwrap();
        assert_eq!(
            b.en(Punto { x: r.x + 5, y: r.y + 5 }),
            Some(EnModos::Repetir)
        );
        assert_eq!(
            b.en(Punto {
                x: b.ancho.x + 3,
                y: b.ancho.y + 3
            }),
            Some(EnModos::Ancho)
        );
        // Nada se solapa: cada boton en su sitio.
        let mut todos: Vec<Rect> = b.modos.iter().map(|(_, r)| *r).collect();
        todos.extend([b.ancho, b.alto, r]);
        todos.extend(b.chips.iter().map(|(_, r)| *r));
        for (i, a) in todos.iter().enumerate() {
            assert!(b.panel.interseccion(*a) == Some(*a), "dentro del panel");
            for c in &todos[i + 1..] {
                assert!(a.interseccion(*c).is_none(), "{a:?} pisa {c:?}");
            }
        }
        // Casos negativos: fuera del panel no es de la barra, y sin zona que
        // repetir no hay boton.
        assert_eq!(b.en(Punto { x: 5, y: 5 }), None);
        let sin = BarraModos::colocar(area(), 100, false);
        assert!(sin.repetir.is_none());
        assert!(sin.panel.ancho <= b.panel.ancho);
    }

    #[test]
    fn la_barra_crece_con_la_escala() {
        let a = BarraModos::colocar(area(), 100, true);
        let b = BarraModos::colocar(area(), 150, true);
        assert!(b.panel.ancho > a.panel.ancho && b.panel.alto > a.panel.alto);
        assert_eq!(b.modos[0].1.ancho, 150);
    }

    #[test]
    fn las_acciones_de_despues_y_sus_atajos() {
        assert_eq!(Accion::de_tecla(0x0D, false, false), Some(Accion::Copiar));
        assert_eq!(Accion::de_tecla(u32::from(b'C'), true, false), Some(Accion::Copiar));
        assert_eq!(Accion::de_tecla(u32::from(b'P'), true, false), Some(Accion::Pinear));
        assert_eq!(Accion::de_tecla(u32::from(b'M'), true, false), Some(Accion::AlChat));
        assert_eq!(Accion::de_tecla(u32::from(b'T'), true, false), Some(Accion::Texto));
        assert_eq!(Accion::de_tecla(u32::from(b'S'), true, false), Some(Accion::Guardar));
        assert_eq!(Accion::de_tecla(0x1B, false, false), Some(Accion::Descartar));
        // Casos negativos: sin Ctrl la letra no es una accion (es un modo o
        // texto), y Ctrl+Mayus+S es «guardar como», que va aparte.
        assert_eq!(Accion::de_tecla(u32::from(b'P'), false, false), None);
        assert_eq!(Accion::de_tecla(u32::from(b'S'), true, true), None);
        assert_eq!(Accion::de_tecla(u32::from(b'Z'), true, false), None);
        // Copiar es la primera, como pide la maqueta.
        assert_eq!(Accion::TODAS[0], Accion::Copiar);
    }

    #[test]
    fn las_herramientas_van_del_uno_al_cinco() {
        assert_eq!(Util::de_tecla(0x31), Some(Util::Lapiz));
        assert_eq!(Util::de_tecla(0x35), Some(Util::Mosaico));
        assert_eq!(Util::de_tecla(0x62), Some(Util::Flecha), "teclado numerico");
        assert_eq!(Util::Texto.numero(), 4);
        // Casos negativos: el 0 y el 6 no son herramientas.
        assert_eq!(Util::de_tecla(0x30), None);
        assert_eq!(Util::de_tecla(0x36), None);
    }

    #[test]
    fn despues_anotar_va_encima_y_acciones_debajo() {
        let sel = Rect {
            x: 240,
            y: 300,
            ancho: 680,
            alto: 340,
        };
        let (a, b) = colocar_despues(sel, area(), 100);
        assert!(a.panel.abajo() <= sel.y);
        assert!(b.panel.y >= sel.abajo());
        assert_eq!(b.panel.x, 240);
        for (acc, r) in &b.botones {
            assert_eq!(b.en(Punto { x: r.x + 4, y: r.y + 4 }), Some(Some(*acc)));
        }
        for (u, r) in &a.utiles {
            assert_eq!(a.en(Punto { x: r.x + 4, y: r.y + 4 }), Some(EnAnotar::Util(*u)));
        }
        assert_eq!(
            a.en(Punto {
                x: a.colores[2].x + 3,
                y: a.colores[2].y + 3
            }),
            Some(EnAnotar::Color(2))
        );
        // Todo dentro de su panel.
        assert!(a.chapa.derecha() <= a.panel.derecha());
        assert!(b.botones.last().unwrap().1.derecha() <= b.panel.derecha());
    }

    #[test]
    fn despues_con_la_zona_a_pantalla_completa_las_barras_entran_en_ella() {
        // Caso negativo de la anterior: no hay sitio fuera.
        let sel = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let (a, b) = colocar_despues(sel, area(), 100);
        assert!(a.panel.y >= 0 && a.panel.abajo() <= 1040);
        assert!(b.panel.y >= 0 && b.panel.abajo() <= 1040);
        assert!(a.panel.interseccion(b.panel).is_none());
    }

    #[test]
    fn despues_sin_sitio_arriba_anotar_baja_tras_las_acciones() {
        let sel = Rect {
            x: 100,
            y: 10,
            ancho: 600,
            alto: 300,
        };
        let (a, b) = colocar_despues(sel, area(), 100);
        assert!(b.panel.y >= sel.abajo());
        assert!(a.panel.y >= b.panel.abajo());
    }

    #[test]
    fn el_selector_cuelga_del_boton_y_reparte_filas() {
        let boton = Rect {
            x: 568,
            y: 498,
            ancho: 156,
            alto: 40,
        };
        let s = Selector::colocar(boton, area(), 4, 100);
        assert!(s.panel.y > boton.abajo());
        assert_eq!(s.filas.len(), 4);
        for (i, f) in s.filas.iter().enumerate() {
            assert!(f.alto >= 44);
            assert_eq!(s.en(Punto { x: f.x + 2, y: f.y + 2 }), Some(EnSelector::Fila(i)));
        }
        assert_eq!(
            s.en(Punto {
                x: s.enviar.x + 2,
                y: s.enviar.y + 2
            }),
            Some(EnSelector::Enviar)
        );
        // Muy abajo, se abre hacia arriba.
        let bajo = Rect { y: 1000, ..boton };
        let s2 = Selector::colocar(bajo, area(), 4, 100);
        assert!(s2.panel.abajo() <= bajo.y);
        // Caso negativo: mas proyectos que filas no estira el panel.
        let s3 = Selector::colocar(boton, area(), 40, 100);
        assert_eq!(s3.filas.len(), FILAS_SELECTOR);
    }

    #[test]
    fn filtrar_proyectos_sin_tildes_ni_mayusculas() {
        let p = vec![
            ("a".to_string(), "Tesis".to_string()),
            ("b".to_string(), "Obra Miraflores".to_string()),
            ("c".to_string(), "Exámenes".to_string()),
        ];
        assert_eq!(filtrar(&p, "").len(), 3);
        assert_eq!(filtrar(&p, "mira")[0].0, "b");
        assert_eq!(filtrar(&p, "EXAMEN")[0].0, "c");
        // Caso negativo.
        assert!(filtrar(&p, "zzz").is_empty());
    }

    #[test]
    fn la_lupa_cambia_de_lado_en_los_bordes_y_centra_el_pixel() {
        let m = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let l = PanelLupa::colocar(Punto { x: 100, y: 100 }, m, 100);
        assert!(l.panel.x > 100 && l.panel.y > 100);
        assert_eq!(l.fuente.0 % 2, 1);
        assert_eq!(l.fuente.1 % 2, 1);
        let r = l.region(Punto { x: 100, y: 100 }, m);
        assert_eq!(r.x + r.ancho as i32 / 2, 100);
        // Caso negativo: junto al borde de abajo a la derecha no se sale.
        let l2 = PanelLupa::colocar(Punto { x: 1910, y: 1070 }, m, 100);
        assert!(l2.panel.derecha() <= 1920 && l2.panel.abajo() <= 1080);
        assert!(l2.panel.x < 1910);
        let r2 = l2.region(Punto { x: 1919, y: 1079 }, m);
        assert!(r2.derecha() <= 1920 && r2.abajo() <= 1080);
    }

    #[test]
    fn las_cifras_solo_admiten_numeros_y_dan_su_valor() {
        let mut c = Cifras::default();
        assert!(c.escribir('6'));
        assert!(c.escribir('2'));
        assert!(c.escribir('0'));
        assert!(!c.escribir('x'), "caso negativo: una letra no entra");
        assert_eq!(c.valor(), Some(620));
        assert!(c.borrar());
        assert_eq!(c.valor(), Some(62));
        let mut cero = Cifras::default();
        cero.escribir('0');
        assert_eq!(cero.valor(), None, "caso negativo: cero no es una medida");
    }

    #[test]
    fn las_medidas_tecleadas_salen_de_la_esquina_o_centradas() {
        let m = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let sel = Rect {
            x: 50,
            y: 60,
            ancho: 10,
            alto: 10,
        };
        assert_eq!(
            zona_de_medidas(800, 600, Some(sel), m),
            Rect {
                x: 50,
                y: 60,
                ancho: 800,
                alto: 600
            }
        );
        assert_eq!(
            zona_de_medidas(800, 600, None, m),
            Rect {
                x: 560,
                y: 240,
                ancho: 800,
                alto: 600
            }
        );
    }

    #[test]
    fn la_etiqueta_de_la_ventana_va_encima_o_dentro() {
        let m = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let r = Rect {
            x: 470,
            y: 200,
            ancho: 620,
            alto: 400,
        };
        let et = etiqueta_de_ventana(r, 400, m, 100);
        assert!(et.abajo() <= r.y);
        // Caso negativo: una ventana maximizada no deja sitio encima.
        let max = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let et2 = etiqueta_de_ventana(max, 400, m, 100);
        assert!(et2.y >= 0);
    }
}
