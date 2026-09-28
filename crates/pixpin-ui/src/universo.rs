//! Donde va cada cosa en la pantalla del universo (§4.4): la barra de ruta,
//! el buscador, el inspector, el minimapa y el selector de emojis.
//!
//! Geometria pura, como la caja de herramientas: decide rectangulos y dice
//! que hay bajo un punto. Pintar y reaccionar es de la app. Asi las medidas
//! de la spec (36, 280, 180 x 120) se prueban sin ventana.

use pixpin_geom::{Punto, Rect};
use pixpin_universo::buscar::normalizar;

/// Medidas en pixeles logicos (al 100 %).
const ALTO_RUTA: u32 = 36;
const ANCHO_BUSCADOR: u32 = 260;
const MARGEN_BUSCADOR: u32 = 6;
const ANCHO_INSPECTOR: u32 = 280;
const ANCHO_MINIMAPA: u32 = 180;
const ALTO_MINIMAPA: u32 = 120;
const MARGEN_MINIMAPA: u32 = 16;

fn a_escala(v: u32, escala_por_cien: u32) -> u32 {
    v * escala_por_cien / 100
}

/// Los paneles de la pantalla del universo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paneles {
    pub ruta: Rect,
    pub buscador: Rect,
    /// Ancho cero sin nada elegido: el inspector solo sale cuando hay que
    /// inspeccionar algo, y el lienzo recupera ese sitio.
    pub inspector: Rect,
    /// Ancho cero si esta escondido (tecla M).
    pub minimapa: Rect,
    /// Lo que queda para el cielo.
    pub lienzo: Rect,
}

impl Paneles {
    pub fn calcular(
        area: Rect,
        escala_por_cien: u32,
        con_inspector: bool,
        con_minimapa: bool,
    ) -> Paneles {
        let e = |v| a_escala(v, escala_por_cien);
        let alto_ruta = e(ALTO_RUTA).min(area.alto);
        let ruta = Rect {
            x: area.x,
            y: area.y,
            ancho: area.ancho,
            alto: alto_ruta,
        };
        let ancho_buscador = e(ANCHO_BUSCADOR).min(area.ancho);
        let buscador = Rect {
            x: area.x + area.ancho as i32 - ancho_buscador as i32 - e(MARGEN_BUSCADOR) as i32,
            y: area.y + e(MARGEN_BUSCADOR) as i32,
            ancho: ancho_buscador,
            alto: alto_ruta.saturating_sub(2 * e(MARGEN_BUSCADOR)),
        };
        let abajo = area.y + alto_ruta as i32;
        let alto_resto = area.alto - alto_ruta;
        let ancho_inspector = if con_inspector {
            e(ANCHO_INSPECTOR).min(area.ancho)
        } else {
            0
        };
        let inspector = Rect {
            x: area.x + (area.ancho - ancho_inspector) as i32,
            y: abajo,
            ancho: ancho_inspector,
            alto: if con_inspector { alto_resto } else { 0 },
        };
        let lienzo = Rect {
            x: area.x,
            y: abajo,
            ancho: area.ancho - ancho_inspector,
            alto: alto_resto,
        };
        let minimapa = if con_minimapa {
            let (w, h) = (e(ANCHO_MINIMAPA), e(ALTO_MINIMAPA));
            Rect {
                x: lienzo.x + lienzo.ancho as i32 - w as i32 - e(MARGEN_MINIMAPA) as i32,
                y: lienzo.y + lienzo.alto as i32 - h as i32 - e(MARGEN_MINIMAPA) as i32,
                ancho: w,
                alto: h,
            }
        } else {
            Rect {
                x: lienzo.x,
                y: lienzo.y,
                ancho: 0,
                alto: 0,
            }
        };
        Paneles {
            ruta,
            buscador,
            inspector,
            minimapa,
            lienzo,
        }
    }
}

/// Las herramientas del universo, en su orden. La figura y el rotulo (H3)
/// van junto al emoji, que es con quien van en el «Anadir» del movil.
pub const HERRAMIENTAS_UNIVERSO: [pixpin_universo::HerramientaUniverso; 5] = [
    pixpin_universo::HerramientaUniverso::Planeta,
    pixpin_universo::HerramientaUniverso::Emoji,
    pixpin_universo::HerramientaUniverso::Figura,
    pixpin_universo::HerramientaUniverso::Rotulo,
    pixpin_universo::HerramientaUniverso::Conectar,
];

/// La isla con las tres herramientas del universo, arriba a la izquierda
/// del cielo, con las medidas de la barra del editor (botones de 36, 4 de
/// hueco y de relleno, a 16 del borde).
///
/// Va aparte de la caja del editor y no como tres botones mas de ella: la
/// caja la comparten el pin y la captura, y un boton nuevo en su lista
/// obligaria a tocar cada sitio que la recorre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarraUniverso {
    pub marco: Rect,
    lado: u32,
    hueco: u32,
    relleno: u32,
}

impl BarraUniverso {
    pub fn colocar(lienzo: Rect, escala_por_cien: u32) -> BarraUniverso {
        let e = |v| a_escala(v, escala_por_cien);
        let (lado, hueco, relleno) = (e(36), e(4), e(4));
        let n = HERRAMIENTAS_UNIVERSO.len() as u32;
        BarraUniverso {
            marco: Rect {
                x: lienzo.x + e(16) as i32,
                y: lienzo.y + e(16) as i32,
                ancho: 2 * relleno + n * lado + (n - 1) * hueco,
                alto: 2 * relleno + lado,
            },
            lado,
            hueco,
            relleno,
        }
    }

    pub fn rect_de(&self, i: usize) -> Rect {
        Rect {
            x: self.marco.x + (self.relleno + i as u32 * (self.lado + self.hueco)) as i32,
            y: self.marco.y + self.relleno as i32,
            ancho: self.lado,
            alto: self.lado,
        }
    }

    pub fn boton_en(&self, p: Punto) -> Option<pixpin_universo::HerramientaUniverso> {
        (0..HERRAMIENTAS_UNIVERSO.len())
            .find(|i| self.rect_de(*i).contiene(p))
            .map(|i| HERRAMIENTAS_UNIVERSO[i])
    }
}

/// Lo que ocupa el «…» y el hueco entre tramos, en las mismas unidades que
/// los anchos que se pasan.
pub const ANCHO_PUNTOS: f32 = 24.0;
pub const HUECO_TRAMO: f32 = 16.0;

/// Donde va cada tramo de la ruta «Cosmos > Galaxia > Planeta > Luna».
///
/// `tramos` son `(texto, ancho ya medido)`. Si no caben, se quitan por la
/// izquierda y se pone «…» delante (su indice es `usize::MAX`): lo mas
/// cercano a lo que se mira es lo que importa. El ultimo sale siempre. Los
/// rectangulos van desde x = 0 y con alto 0: quien pinta los mueve a su
/// barra.
pub fn tramos_de_ruta(ancho: u32, tramos: &[(String, f32)]) -> Vec<(usize, Rect)> {
    if tramos.is_empty() {
        return Vec::new();
    }
    let disponible = ancho as f32;
    let total: f32 = tramos.iter().map(|(_, w)| w + HUECO_TRAMO).sum::<f32>() - HUECO_TRAMO;
    let desde = if total <= disponible {
        0
    } else {
        // Desde la derecha, lo que quepa detras del «…».
        let mut usado = ANCHO_PUNTOS;
        let mut desde = tramos.len() - 1;
        usado += tramos[desde].1;
        while desde > 0 && usado + HUECO_TRAMO + tramos[desde - 1].1 <= disponible {
            desde -= 1;
            usado += HUECO_TRAMO + tramos[desde].1;
        }
        desde
    };
    let mut salida = Vec::new();
    let mut x = 0.0f32;
    if desde > 0 {
        salida.push((
            usize::MAX,
            Rect {
                x: 0,
                y: 0,
                ancho: ANCHO_PUNTOS as u32,
                alto: 0,
            },
        ));
        x = ANCHO_PUNTOS + HUECO_TRAMO;
    }
    for (i, (_, w)) in tramos.iter().enumerate().skip(desde) {
        salida.push((
            i,
            Rect {
                x: x as i32,
                y: 0,
                ancho: *w as u32,
                alto: 0,
            },
        ));
        x += w + HUECO_TRAMO;
    }
    salida
}

/// Que se pulsa en el inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionInspector {
    Abrir,
    IrAlChat,
    MostrarEnCarpeta,
    /// Devolver una luna a la nebulosa.
    Devolver,
    /// Uno de los colores de planeta (`COLORES_PLANETA`).
    Color(usize),
    /// 0, 1 o 2: pequeno, mediano o grande (D223).
    Tamano(u8),
    NotasDelChat,
    Ordenar,
    LimpiarHuerfanas,
    /// La conexion `i` de la lista: cambia de tipo.
    Conexion(usize),
    AgruparNuevo,
    ConectarEntreSi,
}

/// Los colores que se ofrecen para un planeta: los del avatar del chat, que
/// ya se sabe que se leen bien sobre el oscuro.
pub const COLORES_PLANETA: [u32; 7] = [
    0xe17076, 0xfaa774, 0xa695e7, 0x7bc862, 0x6ec9cb, 0x65aadd, 0xee7aae,
];

/// Que se inspecciona: decide que filas hay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FichaInspector {
    Luna,
    Planeta,
    Galaxia,
    /// Varios elegidos a la vez.
    Varios(usize),
}

/// Las filas del inspector, de arriba abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fila {
    Cabecera,
    VistaPrevia,
    /// Una de las cuatro lineas de datos.
    Dato(usize),
    Nota,
    /// La conexion `i`.
    Conexion(usize),
    /// Una fila de colores o de tamanos, o un boton.
    Accion(AccionInspector),
}

/// Las conexiones que se listan como mucho: mas es una lista que no se lee.
pub const CONEXIONES_VISIBLES: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspector {
    pub filas: Vec<(Fila, Rect)>,
}

impl Inspector {
    pub fn colocar(
        marco: Rect,
        escala_por_cien: u32,
        clase: FichaInspector,
        conexiones: usize,
    ) -> Inspector {
        let e = |v| a_escala(v, escala_por_cien);
        let margen = e(12);
        let ancho = marco.ancho.saturating_sub(2 * margen);
        let x = marco.x + margen as i32;
        let mut y = marco.y + margen as i32;
        let mut filas = Vec::new();
        let mut fila = |f: Fila, alto: u32, y: &mut i32| {
            filas.push((
                f,
                Rect {
                    x,
                    y: *y,
                    ancho,
                    alto,
                },
            ));
            *y += alto as i32 + margen as i32 / 2;
        };
        fila(Fila::Cabecera, e(56), &mut y);
        if clase == FichaInspector::Luna {
            fila(Fila::VistaPrevia, e(160), &mut y);
        }
        if !matches!(clase, FichaInspector::Varios(_)) {
            for i in 0..4 {
                fila(Fila::Dato(i), e(20), &mut y);
            }
            fila(Fila::Nota, e(96), &mut y);
            for i in 0..conexiones.min(CONEXIONES_VISIBLES) {
                fila(Fila::Conexion(i), e(28), &mut y);
            }
        }
        let botones: &[AccionInspector] = match clase {
            FichaInspector::Luna => &[
                AccionInspector::Abrir,
                AccionInspector::IrAlChat,
                AccionInspector::MostrarEnCarpeta,
                AccionInspector::Devolver,
            ],
            FichaInspector::Planeta => &[AccionInspector::Abrir],
            FichaInspector::Galaxia => &[
                AccionInspector::IrAlChat,
                AccionInspector::NotasDelChat,
                AccionInspector::Ordenar,
                AccionInspector::LimpiarHuerfanas,
            ],
            FichaInspector::Varios(_) => &[
                AccionInspector::AgruparNuevo,
                AccionInspector::ConectarEntreSi,
            ],
        };
        if clase == FichaInspector::Planeta {
            // Los colores y los tamanos, en una fila cada uno: cajitas del
            // mismo ancho, una por opcion.
            let lado = e(32);
            let n = COLORES_PLANETA.len() as u32;
            let paso = ancho / n.max(1);
            for i in 0..COLORES_PLANETA.len() {
                filas.push((
                    Fila::Accion(AccionInspector::Color(i)),
                    Rect {
                        x: x + (i as u32 * paso) as i32,
                        y,
                        ancho: paso.saturating_sub(e(4)),
                        alto: lado,
                    },
                ));
            }
            y += lado as i32 + margen as i32 / 2;
            let paso = ancho / 3;
            for t in 0..3u8 {
                filas.push((
                    Fila::Accion(AccionInspector::Tamano(t)),
                    Rect {
                        x: x + (t as u32 * paso) as i32,
                        y,
                        ancho: paso.saturating_sub(e(4)),
                        alto: lado,
                    },
                ));
            }
            y += lado as i32 + margen as i32 / 2;
        }
        for b in botones {
            filas.push((
                Fila::Accion(*b),
                Rect {
                    x,
                    y,
                    ancho,
                    alto: e(32),
                },
            ));
            y += e(32) as i32 + margen as i32 / 2;
        }
        Inspector { filas }
    }

    pub fn accion_en(&self, p: Punto) -> Option<AccionInspector> {
        self.filas.iter().find_map(|(f, r)| match f {
            Fila::Accion(a) if r.contiene(p) => Some(*a),
            Fila::Conexion(i) if r.contiene(p) => Some(AccionInspector::Conexion(*i)),
            _ => None,
        })
    }

    /// La caja de la nota, o una vacia si esta ficha no la tiene.
    pub fn caja_nota(&self) -> Rect {
        self.filas
            .iter()
            .find(|(f, _)| *f == Fila::Nota)
            .map(|(_, r)| *r)
            .unwrap_or(Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            })
    }
}

/// La escala uniforme y el desplazamiento que meten `cosmos` entero y
/// centrado en `mapa`, sin deformar.
fn encaje(mapa: Rect, cosmos: (f32, f32, f32, f32)) -> (f32, f32, f32) {
    let (w, h) = (
        (cosmos.2 - cosmos.0).max(1.0),
        (cosmos.3 - cosmos.1).max(1.0),
    );
    let s = (mapa.ancho as f32 / w).min(mapa.alto as f32 / h);
    let dx = mapa.x as f32 + (mapa.ancho as f32 - w * s) / 2.0;
    let dy = mapa.y as f32 + (mapa.alto as f32 - h * s) / 2.0;
    (s, dx, dy)
}

/// Un punto del minimapa, en el mundo: donde se va al pulsarlo.
pub fn minimapa_a_mundo(mapa: Rect, cosmos: (f32, f32, f32, f32), p: Punto) -> (f32, f32) {
    let (s, dx, dy) = encaje(mapa, cosmos);
    (
        cosmos.0 + (p.x as f32 - dx) / s,
        cosmos.1 + (p.y as f32 - dy) / s,
    )
}

/// Un punto del mundo, en el minimapa: donde se pinta.
pub fn mundo_a_minimapa(mapa: Rect, cosmos: (f32, f32, f32, f32), q: (f32, f32)) -> (f32, f32) {
    let (s, dx, dy) = encaje(mapa, cosmos);
    (dx + (q.0 - cosmos.0) * s, dy + (q.1 - cosmos.1) * s)
}

/// Las pestanas del selector de emojis (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pestana {
    #[default]
    Espacio,
    Caras,
    Objetos,
    Naturaleza,
    Simbolos,
    /// Los 16 ultimos usados: se rellena en ejecucion.
    Recientes,
}

pub const PESTANAS: [Pestana; 6] = [
    Pestana::Espacio,
    Pestana::Caras,
    Pestana::Objetos,
    Pestana::Naturaleza,
    Pestana::Simbolos,
    Pestana::Recientes,
];

/// Cuantos recientes se recuerdan.
pub const RECIENTES: usize = 16;

use Pestana::{Caras as C, Espacio as E, Naturaleza as N, Objetos as O, Simbolos as S};

/// `(emoji, nombre en espanol, nombre en ingles, pestana)`. Los nombres son
/// para el filtro: se busca en los dos idiomas a la vez, porque mucha gente
/// conoce el nombre ingles de un emoji aunque escriba en espanol.
pub const EMOJIS: [(&str, &str, &str, Pestana); 200] = [
    ("🪐", "planeta saturno", "planet ringed", E),
    ("🌌", "galaxia via lactea", "galaxy milky way", E),
    ("🌍", "tierra mundo europa africa", "earth globe world", E),
    ("🌎", "tierra mundo america", "earth globe americas", E),
    ("🌏", "tierra mundo asia", "earth globe asia", E),
    ("🌕", "luna llena", "full moon", E),
    ("🌖", "luna menguante gibosa", "waning gibbous moon", E),
    ("🌗", "cuarto menguante", "last quarter moon", E),
    ("🌘", "luna menguante", "waning crescent moon", E),
    ("🌑", "luna nueva", "new moon", E),
    ("🌒", "luna creciente", "waxing crescent moon", E),
    ("🌓", "cuarto creciente", "first quarter moon", E),
    ("🌔", "luna creciente gibosa", "waxing gibbous moon", E),
    ("🌙", "luna", "crescent moon", E),
    ("🌚", "luna nueva cara", "new moon face", E),
    (
        "🌛",
        "luna cuarto creciente cara",
        "first quarter moon face",
        E,
    ),
    (
        "🌜",
        "luna cuarto menguante cara",
        "last quarter moon face",
        E,
    ),
    ("🌝", "luna llena cara", "full moon face", E),
    ("⭐", "estrella", "star", E),
    ("🌟", "estrella brillante", "glowing star", E),
    ("✨", "chispas brillos", "sparkles", E),
    ("💫", "mareo estrella", "dizzy star", E),
    ("🌠", "estrella fugaz", "shooting star", E),
    ("☄️", "cometa", "comet", E),
    ("🚀", "cohete", "rocket", E),
    ("🛰️", "satelite", "satellite", E),
    ("🛸", "ovni platillo", "ufo flying saucer", E),
    ("👽", "extraterrestre alien", "alien", E),
    ("👾", "marciano juego", "space invader monster", E),
    ("🔭", "telescopio", "telescope", E),
    ("🌞", "sol cara", "sun face", E),
    ("☀️", "sol", "sun", E),
    ("🌃", "noche estrellada", "night stars", E),
    ("🧑‍🚀", "astronauta", "astronaut", E),
    ("🌄", "amanecer montana", "sunrise mountains", E),
    ("🌇", "atardecer ciudad", "sunset city", E),
    ("🌆", "ciudad anochecer", "cityscape dusk", E),
    ("🎆", "fuegos artificiales", "fireworks", E),
    ("🎇", "bengala", "sparkler", E),
    ("🌋", "volcan", "volcano", E),
    ("😀", "cara sonriente", "grinning face", C),
    ("😃", "sonrisa ojos grandes", "grinning big eyes", C),
    ("😄", "sonrisa", "smile", C),
    ("😁", "sonrisa radiante", "beaming", C),
    ("😆", "risa", "laughing", C),
    ("😅", "risa sudor", "sweat smile", C),
    ("😂", "lagrimas de risa", "tears of joy", C),
    ("🙂", "leve sonrisa", "slight smile", C),
    ("😉", "guino", "wink", C),
    ("😊", "sonrojado", "blush", C),
    ("😇", "angel aureola", "halo innocent", C),
    ("😍", "enamorado", "heart eyes", C),
    ("🤩", "deslumbrado estrellas", "star struck", C),
    ("😘", "beso", "kiss", C),
    ("😋", "delicioso", "yum", C),
    ("😜", "lengua guino", "wink tongue", C),
    ("🤔", "pensando", "thinking", C),
    ("🤨", "ceja levantada", "raised eyebrow", C),
    ("😐", "neutral", "neutral", C),
    ("😑", "inexpresivo", "expressionless", C),
    ("😶", "sin boca", "no mouth", C),
    ("🙄", "ojos en blanco", "eye roll", C),
    ("😏", "sonrisa picara", "smirk", C),
    ("😴", "dormido", "sleeping", C),
    ("😷", "mascarilla", "mask", C),
    ("🤒", "termometro enfermo", "sick thermometer", C),
    ("🤯", "cabeza explota", "mind blown", C),
    ("😎", "gafas de sol", "sunglasses cool", C),
    ("🤓", "empollon", "nerd", C),
    ("🧐", "monoculo", "monocle", C),
    ("😕", "confundido", "confused", C),
    ("😟", "preocupado", "worried", C),
    ("😮", "boca abierta", "open mouth", C),
    ("😲", "asombrado", "astonished", C),
    ("😢", "llorando", "crying", C),
    ("😭", "llanto", "sobbing", C),
    ("😱", "grito miedo", "scream", C),
    ("😡", "enfadado", "angry pouting", C),
    ("🥳", "fiesta", "party face", C),
    ("🥺", "suplicante", "pleading", C),
    ("📁", "carpeta", "folder", O),
    ("📂", "carpeta abierta", "open folder", O),
    ("📄", "documento pagina", "document page", O),
    ("📋", "portapapeles", "clipboard", O),
    ("📌", "chincheta", "pushpin", O),
    ("📎", "clip", "paperclip", O),
    ("✏️", "lapiz", "pencil", O),
    ("🖊️", "boligrafo", "pen", O),
    ("📐", "escuadra", "triangular ruler", O),
    ("📏", "regla", "ruler", O),
    ("🔑", "llave", "key", O),
    ("🔒", "candado cerrado", "locked", O),
    ("🔓", "candado abierto", "unlocked", O),
    ("🔨", "martillo", "hammer", O),
    ("🔧", "llave inglesa", "wrench", O),
    ("🧰", "caja de herramientas", "toolbox", O),
    ("💡", "bombilla idea", "light bulb idea", O),
    ("🔋", "bateria", "battery", O),
    ("💻", "portatil", "laptop", O),
    ("🖥️", "ordenador", "desktop computer", O),
    ("📱", "movil telefono", "mobile phone", O),
    ("📷", "camara foto", "camera", O),
    ("🎥", "camara de cine video", "movie camera", O),
    ("📦", "paquete caja", "package box", O),
    ("🧾", "recibo factura", "receipt", O),
    ("💰", "bolsa de dinero", "money bag", O),
    ("💳", "tarjeta", "credit card", O),
    ("📅", "calendario", "calendar", O),
    ("⏰", "despertador", "alarm clock", O),
    ("⌛", "reloj de arena", "hourglass", O),
    ("🎁", "regalo", "gift", O),
    ("🏠", "casa", "house home", O),
    ("🏢", "oficina edificio", "office building", O),
    ("🚗", "coche", "car", O),
    ("✈️", "avion", "airplane", O),
    ("🚲", "bicicleta", "bicycle", O),
    ("📚", "libros", "books", O),
    ("🎨", "paleta pintura", "palette art", O),
    ("🎵", "nota musical", "music note", O),
    ("🧪", "tubo de ensayo", "test tube", O),
    ("🌳", "arbol", "tree", N),
    ("🌲", "pino", "evergreen tree", N),
    ("🌴", "palmera", "palm tree", N),
    ("🌵", "cactus", "cactus", N),
    ("🌷", "tulipan", "tulip", N),
    ("🌹", "rosa", "rose", N),
    ("🌻", "girasol", "sunflower", N),
    ("🌼", "flor", "blossom flower", N),
    ("🍀", "trebol", "four leaf clover", N),
    ("🍁", "hoja de arce", "maple leaf", N),
    ("🍂", "hojas caidas", "fallen leaves", N),
    ("🌾", "espiga arroz", "sheaf of rice", N),
    ("🌿", "hierba", "herb", N),
    ("🍄", "seta", "mushroom", N),
    ("🐶", "perro", "dog", N),
    ("🐱", "gato", "cat", N),
    ("🐭", "raton", "mouse", N),
    ("🐰", "conejo", "rabbit", N),
    ("🦊", "zorro", "fox", N),
    ("🐻", "oso", "bear", N),
    ("🐼", "panda", "panda", N),
    ("🐨", "koala", "koala", N),
    ("🐯", "tigre", "tiger", N),
    ("🦁", "leon", "lion", N),
    ("🐮", "vaca", "cow", N),
    ("🐷", "cerdo", "pig", N),
    ("🐸", "rana", "frog", N),
    ("🐵", "mono", "monkey", N),
    ("🐔", "gallina", "chicken", N),
    ("🐧", "pinguino", "penguin", N),
    ("🐦", "pajaro", "bird", N),
    ("🦋", "mariposa", "butterfly", N),
    ("🐝", "abeja", "bee", N),
    ("🐞", "mariquita", "ladybug", N),
    ("🐢", "tortuga", "turtle", N),
    ("🐍", "serpiente", "snake", N),
    ("🐟", "pez", "fish", N),
    ("🐬", "delfin", "dolphin", N),
    ("🐳", "ballena", "whale", N),
    ("🌈", "arcoiris", "rainbow", N),
    ("❤️", "corazon rojo", "red heart", S),
    ("🧡", "corazon naranja", "orange heart", S),
    ("💛", "corazon amarillo", "yellow heart", S),
    ("💚", "corazon verde", "green heart", S),
    ("💙", "corazon azul", "blue heart", S),
    ("💜", "corazon morado", "purple heart", S),
    ("🖤", "corazon negro", "black heart", S),
    ("✅", "hecho visto", "check done", S),
    ("❌", "cruz no", "cross mark no", S),
    ("⚠️", "aviso cuidado", "warning", S),
    ("❗", "exclamacion", "exclamation", S),
    ("❓", "pregunta", "question", S),
    ("💯", "cien", "hundred points", S),
    ("➕", "mas sumar", "plus", S),
    ("➖", "menos restar", "minus", S),
    ("✖️", "por multiplicar", "multiply", S),
    ("➗", "dividir", "divide", S),
    ("🔴", "circulo rojo", "red circle", S),
    ("🟠", "circulo naranja", "orange circle", S),
    ("🟡", "circulo amarillo", "yellow circle", S),
    ("🟢", "circulo verde", "green circle", S),
    ("🔵", "circulo azul", "blue circle", S),
    ("🟣", "circulo morado", "purple circle", S),
    ("⚫", "circulo negro", "black circle", S),
    ("⚪", "circulo blanco", "white circle", S),
    ("🔺", "triangulo arriba", "red triangle up", S),
    ("🔻", "triangulo abajo", "red triangle down", S),
    ("🔷", "rombo azul", "blue diamond", S),
    ("🔶", "rombo naranja", "orange diamond", S),
    ("⬆️", "flecha arriba", "up arrow", S),
    ("⬇️", "flecha abajo", "down arrow", S),
    ("⬅️", "flecha izquierda", "left arrow", S),
    ("➡️", "flecha derecha", "right arrow", S),
    ("🔁", "repetir", "repeat", S),
    ("🔔", "campana", "bell", S),
    ("🏁", "meta bandera cuadros", "finish flag", S),
    ("🚩", "bandera", "flag", S),
    ("♻️", "reciclar", "recycle", S),
    ("⭕", "circulo hueco", "hollow circle", S),
    ("🆕", "nuevo", "new", S),
];

/// El selector de emojis flotante (§4.5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectorEmojis {
    pub pestana: Pestana,
    pub filtro: String,
    /// Los ultimos usados, el mas reciente primero.
    pub recientes: Vec<String>,
    marco: Option<Rect>,
    escala_por_cien: u32,
}

/// Cuantos emojis por fila, y el lado de cada celda, en logicos.
const COLUMNAS_EMOJI: u32 = 8;
const LADO_CELDA_EMOJI: u32 = 36;
const ALTO_PESTANAS: u32 = 32;
const ALTO_FILTRO: u32 = 32;

impl SelectorEmojis {
    /// Lo que se ve ahora: con filtro, lo que case en cualquier pestana; sin
    /// el, la pestana elegida.
    pub fn visibles(&self) -> Vec<&'static str> {
        let filtro = normalizar(self.filtro.trim());
        if !filtro.is_empty() {
            return EMOJIS
                .iter()
                .filter(|(_, es, en, _)| {
                    normalizar(es).contains(&filtro) || normalizar(en).contains(&filtro)
                })
                .map(|e| e.0)
                .collect();
        }
        if self.pestana == Pestana::Recientes {
            // Solo los que estan en la tabla: los demas no tendrian `'static`
            // y, ademas, no los puso este selector.
            return self
                .recientes
                .iter()
                .filter_map(|r| EMOJIS.iter().find(|e| e.0 == r.as_str()).map(|e| e.0))
                .collect();
        }
        EMOJIS
            .iter()
            .filter(|e| e.3 == self.pestana)
            .map(|e| e.0)
            .collect()
    }

    /// Anota un emoji como el ultimo usado.
    pub fn usar(&mut self, e: &str) {
        self.recientes.retain(|r| r != e);
        self.recientes.insert(0, e.to_string());
        self.recientes.truncate(RECIENTES);
    }

    /// El marco del selector, bajo la caja de herramientas y centrado.
    pub fn colocar(&mut self, lienzo: Rect, escala_por_cien: u32) -> Rect {
        let e = |v| a_escala(v, escala_por_cien);
        let ancho = e(COLUMNAS_EMOJI * LADO_CELDA_EMOJI) + 2 * e(8);
        let alto = e(ALTO_PESTANAS + ALTO_FILTRO + 5 * LADO_CELDA_EMOJI) + 2 * e(8);
        let marco = Rect {
            x: lienzo.x + (lienzo.ancho as i32 - ancho as i32) / 2,
            y: lienzo.y + e(72) as i32,
            ancho,
            alto,
        };
        self.marco = Some(marco);
        self.escala_por_cien = escala_por_cien;
        marco
    }

    pub fn marco(&self) -> Option<Rect> {
        self.marco
    }

    /// Las cajas de las pestanas.
    pub fn pestanas(&self) -> Vec<(Pestana, Rect)> {
        let Some(m) = self.marco else {
            return Vec::new();
        };
        let e = |v| a_escala(v, self.escala_por_cien);
        let ancho = (m.ancho - 2 * e(8)) / PESTANAS.len() as u32;
        PESTANAS
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    *p,
                    Rect {
                        x: m.x + e(8) as i32 + (i as u32 * ancho) as i32,
                        y: m.y + e(8) as i32,
                        ancho,
                        alto: e(ALTO_PESTANAS),
                    },
                )
            })
            .collect()
    }

    /// La caja donde se ve lo que se teclea para filtrar.
    pub fn caja_filtro(&self) -> Option<Rect> {
        let m = self.marco?;
        let e = |v| a_escala(v, self.escala_por_cien);
        Some(Rect {
            x: m.x + e(8) as i32,
            y: m.y + e(8 + ALTO_PESTANAS) as i32,
            ancho: m.ancho - 2 * e(8),
            alto: e(ALTO_FILTRO),
        })
    }

    /// Las celdas de los emojis que se ven, hasta llenar el marco.
    pub fn celdas(&self) -> Vec<(&'static str, Rect)> {
        let Some(m) = self.marco else {
            return Vec::new();
        };
        let e = |v| a_escala(v, self.escala_por_cien);
        let lado = e(LADO_CELDA_EMOJI);
        let y0 = m.y + e(8 + ALTO_PESTANAS + ALTO_FILTRO) as i32;
        let filas = (m.y + m.alto as i32 - e(8) as i32 - y0).max(0) as u32 / lado.max(1);
        self.visibles()
            .into_iter()
            .take((filas * COLUMNAS_EMOJI) as usize)
            .enumerate()
            .map(|(i, emoji)| {
                let (col, fila) = (i as u32 % COLUMNAS_EMOJI, i as u32 / COLUMNAS_EMOJI);
                (
                    emoji,
                    Rect {
                        x: m.x + e(8) as i32 + (col * lado) as i32,
                        y: y0 + (fila * lado) as i32,
                        ancho: lado,
                        alto: lado,
                    },
                )
            })
            .collect()
    }

    pub fn emoji_en(&self, p: Punto) -> Option<&'static str> {
        self.celdas()
            .into_iter()
            .find(|(_, r)| r.contiene(p))
            .map(|(e, _)| e)
    }

    pub fn pestana_en(&self, p: Punto) -> Option<Pestana> {
        self.pestanas()
            .into_iter()
            .find(|(_, r)| r.contiene(p))
            .map(|(t, _)| t)
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
            alto: 1080,
        }
    }

    #[test]
    fn los_paneles_miden_lo_de_la_spec_a_escala() {
        let p = Paneles::calcular(area(), 150, true, true);
        assert_eq!(p.ruta.alto, 54); // 36 * 1,5
        assert_eq!(p.inspector.ancho, 420); // 280 * 1,5
        assert_eq!((p.minimapa.ancho, p.minimapa.alto), (270, 180));
        assert_eq!(p.lienzo.ancho, 1920 - 420);
    }

    #[test]
    fn sin_seleccion_no_hay_inspector_y_el_lienzo_ocupa_todo() {
        let p = Paneles::calcular(area(), 100, false, true);
        assert_eq!(p.inspector.ancho, 0);
        assert_eq!(p.lienzo.ancho, 1920);
    }

    #[test]
    fn la_barra_del_universo_tiene_sus_cinco_herramientas_en_orden() {
        use pixpin_universo::HerramientaUniverso as H;
        let p = Paneles::calcular(area(), 100, false, true);
        let b = BarraUniverso::colocar(p.lienzo, 100);
        let centro = |i: usize| {
            let r = b.rect_de(i);
            Punto {
                x: r.x + r.ancho as i32 / 2,
                y: r.y + r.alto as i32 / 2,
            }
        };
        assert_eq!(b.boton_en(centro(0)), Some(H::Planeta));
        assert_eq!(b.boton_en(centro(1)), Some(H::Emoji));
        assert_eq!(b.boton_en(centro(2)), Some(H::Figura));
        assert_eq!(b.boton_en(centro(3)), Some(H::Rotulo));
        assert_eq!(b.boton_en(centro(4)), Some(H::Conectar));
        // Caso negativo: fuera de la isla, nada; y la isla no pisa la ruta.
        assert_eq!(b.boton_en(Punto { x: 900, y: 900 }), None);
        assert!(b.marco.y >= p.ruta.y + p.ruta.alto as i32);
    }

    #[test]
    fn el_minimapa_escondido_no_ocupa_nada_y_no_se_puede_pulsar() {
        let p = Paneles::calcular(area(), 100, false, false);
        assert_eq!(p.minimapa.ancho, 0);
        assert!(!p.minimapa.contiene(Punto { x: 1800, y: 1000 }));
    }

    #[test]
    fn el_minimapa_va_y_vuelve_entre_pantalla_y_mundo() {
        let mapa = Rect {
            x: 100,
            y: 100,
            ancho: 180,
            alto: 120,
        };
        let cosmos = (-9000.0, -6000.0, 9000.0, 6000.0);
        let (x, y) = minimapa_a_mundo(mapa, cosmos, pixpin_geom::Punto { x: 190, y: 160 });
        assert!((x - 0.0).abs() < 1.0 && (y - 0.0).abs() < 1.0);
        let (px, py) = mundo_a_minimapa(mapa, cosmos, (9000.0, 6000.0));
        assert!((px - 280.0).abs() < 0.5 && (py - 220.0).abs() < 0.5);
    }

    #[test]
    fn el_selector_filtra_por_nombre_en_los_dos_idiomas() {
        let mut s = SelectorEmojis {
            filtro: "planeta".into(),
            ..Default::default()
        };
        assert!(s.visibles().contains(&"🪐"));
        s.filtro = "planet".into();
        assert!(s.visibles().contains(&"🪐"));
        s.filtro = "zzzz".into();
        assert!(s.visibles().is_empty());
    }

    #[test]
    fn hay_doscientos_emojis_sin_repetir() {
        let mut v: Vec<&str> = EMOJIS.iter().map(|e| e.0).collect();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), EMOJIS.len());
        assert!(EMOJIS.len() >= 200);
    }

    #[test]
    fn la_pestana_espacio_trae_los_de_la_spec() {
        let s = SelectorEmojis::default();
        let v = s.visibles();
        for e in [
            "🪐", "🌌", "🌍", "🌕", "🌙", "⭐", "🌟", "✨", "☄️", "🚀", "🛰️", "👽", "🔭", "🌠",
            "🌞", "🌑",
        ] {
            assert!(v.contains(&e), "falta {e}");
        }
    }

    #[test]
    fn los_recientes_van_primero_sin_repetir_y_con_tope() {
        let mut s = SelectorEmojis::default();
        for e in EMOJIS.iter().take(20) {
            s.usar(e.0);
        }
        s.usar(EMOJIS[5].0);
        assert_eq!(s.recientes.len(), RECIENTES);
        assert_eq!(s.recientes[0], EMOJIS[5].0);
        s.pestana = Pestana::Recientes;
        assert_eq!(s.visibles()[0], EMOJIS[5].0);
        // Caso negativo: algo que no esta en la tabla no sale.
        s.recientes = vec!["no-es-un-emoji".into()];
        assert!(s.visibles().is_empty());
    }

    #[test]
    fn un_clic_en_una_celda_da_su_emoji_y_fuera_nada() {
        let mut s = SelectorEmojis::default();
        let m = s.colocar(area(), 100);
        let (e, r) = s.celdas()[0];
        assert_eq!(
            s.emoji_en(Punto {
                x: r.x + 2,
                y: r.y + 2
            }),
            Some(e)
        );
        assert_eq!(
            s.emoji_en(Punto {
                x: m.x - 10,
                y: m.y
            }),
            None
        );
    }

    #[test]
    fn la_ruta_que_no_cabe_se_recorta_por_la_izquierda() {
        let tramos: Vec<(String, f32)> = (0..10).map(|i| (format!("tramo{i}"), 200.0)).collect();
        let v = tramos_de_ruta(600, &tramos);
        assert_eq!(v.last().unwrap().0, 9, "el ultimo tramo siempre se ve");
        assert!(v.len() < 10);
        assert_eq!(v[0].0, usize::MAX, "delante va el «…»");
        // Caso negativo: si cabe, no hay «…».
        let pocos = tramos_de_ruta(2000, &tramos[..3]);
        assert_eq!(pocos.iter().map(|t| t.0).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn el_inspector_de_una_luna_tiene_vista_previa_y_sus_cuatro_botones() {
        let marco = Rect {
            x: 1640,
            y: 36,
            ancho: 280,
            alto: 1044,
        };
        let i = Inspector::colocar(marco, 100, FichaInspector::Luna, 2);
        assert!(i.filas.iter().any(|(f, _)| *f == Fila::VistaPrevia));
        let abrir = i
            .filas
            .iter()
            .find(|(f, _)| *f == Fila::Accion(AccionInspector::Abrir))
            .unwrap()
            .1;
        assert_eq!(
            i.accion_en(Punto {
                x: abrir.x + 3,
                y: abrir.y + 3
            }),
            Some(AccionInspector::Abrir)
        );
        assert!(i.caja_nota().alto > 0);
        // Caso negativo: una galaxia no tiene vista previa ni «Devolver».
        let g = Inspector::colocar(marco, 100, FichaInspector::Galaxia, 0);
        assert!(!g.filas.iter().any(|(f, _)| *f == Fila::VistaPrevia));
        assert!(
            !g.filas
                .iter()
                .any(|(f, _)| *f == Fila::Accion(AccionInspector::Devolver))
        );
    }
}
