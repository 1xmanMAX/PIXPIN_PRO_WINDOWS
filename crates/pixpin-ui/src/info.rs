//! La ficha de un chat: lo que sale al pulsar su nombre en la cabecera.
//!
//! Geometria pura, como `chat` e `historial`. Las medidas salen del analisis
//! de Telegram Desktop; **de ahi solo medidas y tecnicas, nunca su codigo,
//! que es GPL-3.0.**
//!
//! Las SECCIONES, en cambio, viven en `pixpin_proyecto::cuaderno::Seccion`:
//! las de PixPin Android (todo, fotos, archivos, voz, dibujos, fijados y
//! buzon) y, desde el 25-oct-2026, videos y enlaces, como Telegram. La
//! disposicion es la suya; lo que se lista es lo nuestro.
//!
//! La ficha es la **isla** (`isla_en`, [`Isla`], `previa`). La columna y la
//! capa de antes se quitaron cuando nadie las llamaba ya (25-oct-2026).
//!
//! Tres decisiones que no son evidentes:
//!
//! - **La pildora de la pestana activa NO es una rayita.** En Telegram es una
//!   pildora rellena que se desliza Y cambia de ancho entre pestana y
//!   pestana. Una rayita es mas facil y se ve distinto.
//! - **La cuadricula se centra.** Se calcula cuantas columnas caben con la
//!   celda minima, se reparte el sobrante entre ellas y el margen se recalcula
//!   para centrarla. Sin eso sobra un pico a la derecha que canta.
//! - **Las celdas son cuadradas.** Una foto apaisada y una vertical ocupan lo
//!   mismo; es lo que hace que una cuadricula se lea de un vistazo.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
///
/// Lo alta que es la caja de buscar dentro de la cabecera, y la letra que
/// lleva. La misma que el buscador de la lista de chats: es la misma clase
/// de caja y desigualarlas solo se notaria para mal.
pub const BUSCAR_ALTO: u32 = 32;
pub const BUSCAR_TAM: f32 = 13.0;
pub const BUSCAR_TEXTO_X: u32 = 12;
/// Lo que se separa la pildora del borde de su pestana.
pub const PILDORA_HUECO: u32 = 4;

/// Como queda la cuadricula de fotos en un ancho dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejilla {
    pub columnas: u32,
    /// El lado de una celda: son cuadradas.
    pub lado: u32,
    /// Donde empieza la primera columna, ya centrada.
    pub margen: u32,
    /// Lo que se deja entre dos celdas, ya escalado.
    pub hueco: u32,
}

/// Reparte el ancho en columnas cuadradas de al menos `minima` de lado, con
/// `hueco` entre ellas y `margen` a cada lado (logicos, sin escalar).
pub fn rejilla_con(
    ancho: u32,
    minima: u32,
    hueco: u32,
    margen: u32,
    escala_por_cien: u32,
) -> Rejilla {
    let e = |v: u32| v * escala_por_cien / 100;
    let hueco = e(hueco);
    let minima = e(minima).max(1);
    let util = ancho.saturating_sub(2 * e(margen));
    // Cuantas caben, contando que entre dos hay un hueco.
    let columnas = ((util + hueco) / (minima + hueco)).max(1);
    let lado = ((util + hueco) / columnas).saturating_sub(hueco).max(1);
    let ocupa = columnas * lado + (columnas - 1) * hueco;
    Rejilla {
        columnas,
        lado,
        margen: ancho.saturating_sub(ocupa) / 2,
        hueco,
    }
}

impl Rejilla {
    /// La celda numero `indice`, con el desplazamiento ya restado.
    pub fn celda(&self, indice: usize, area: Rect, scroll: i32) -> Rect {
        let hueco = self.hueco;
        let fila = indice as u32 / self.columnas;
        let columna = indice as u32 % self.columnas;
        Rect {
            x: area.x + self.margen as i32 + (columna * (self.lado + hueco)) as i32,
            y: area.y + (fila * (self.lado + hueco)) as i32 - scroll,
            ancho: self.lado,
            alto: self.lado,
        }
    }

    /// Lo que ocupa todo junto.
    pub fn alto_total(&self, cuantas: usize) -> u32 {
        if cuantas == 0 {
            return 0;
        }
        let hueco = self.hueco;
        let filas = cuantas.div_ceil(self.columnas as usize) as u32;
        filas * self.lado + (filas - 1) * hueco
    }

    /// Que celdas se ven, para pintar solo esas.
    pub fn visibles(&self, area: Rect, cuantas: usize, scroll: i32) -> (usize, usize) {
        let hueco = self.hueco;
        let paso = (self.lado + hueco).max(1) as i32;
        let primera_fila = (scroll / paso).max(0) as usize;
        let filas_que_caben = (area.alto as i32 / paso + 2) as usize;
        let primera = primera_fila * self.columnas as usize;
        let cuantas_ver = filas_que_caben * self.columnas as usize;
        (
            primera.min(cuantas),
            cuantas_ver.min(cuantas.saturating_sub(primera)),
        )
    }
}

/// Lo de dentro de una fila: su miniatura y sus dos lineas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilaArchivo {
    pub fila: Rect,
    pub miniatura: Rect,
    pub nombre: Punto,
    pub estado: Punto,
    pub ancho_texto: u32,
}

// --- La isla de la ficha del chat (25-oct-2026) -----------------------------
//
// El usuario: «cuando hago click en la cabecera del chat me sale Galeria,
// Archivos, Voz… su ui es mala. Mira Telegram: es una isla flotante que
// muestra fotos, videos, archivos, enlaces… muy bien presentado, y hacer
// click dentro no debe salir de esa zona». Es la misma capa con velo, pero
// mas ancha y que se adapta a la ventana: la rejilla pone mas columnas de
// miniaturas cuanto mas sitio hay, y lo que se pulsa dentro se ve DENTRO
// (`Previa`), con su flecha de volver a la rejilla.

/// Lo estrecha y lo ancha que puede ser la isla.
pub const ISLA_ANCHO_MINIMO: u32 = 360;
pub const ISLA_ANCHO_MAXIMO: u32 = 720;
/// La parte del ancho de la ventana que se lleva (en por ciento).
pub const ISLA_FRACCION: u32 = 58;
/// Ni en un monitor altisimo se estira mas de esto.
pub const ISLA_ALTO_MAXIMO: u32 = 860;
/// Lo que se deja como minimo a los lados, y arriba y abajo.
pub const ISLA_MARGEN_X: u32 = 32;
pub const ISLA_MARGEN_Y: u32 = 28;
pub const ISLA_RADIO: u32 = 16;
/// La cabecera: el avatar del chat, su nombre y lo que tiene; la lupa y el
/// aspa a la derecha.
pub const ISLA_CABECERA: u32 = 68;
pub const ISLA_AVATAR: u32 = 42;
pub const ISLA_AVATAR_X: u32 = 16;
pub const ISLA_BOTON: u32 = 40;
pub const ISLA_TITULO_TAM: f32 = 15.0;
pub const ISLA_SUBTITULO_TAM: f32 = 12.5;
/// La tira de pestanas, un selector segmentado.
pub const ISLA_TIRA: u32 = 48;
pub const ISLA_TIRA_MARGEN_X: u32 = 12;
pub const ISLA_TIRA_MARGEN_Y: u32 = 6;
/// El relleno de cada pestana, y lo minimo que se le deja si no caben.
pub const ISLA_PESTANA_RELLENO: u32 = 14;
pub const ISLA_PESTANA_RELLENO_MINIMO: u32 = 6;
pub const ISLA_PESTANA_TAM: f32 = 13.0;
/// La rejilla: miniaturas cuadradas de al menos esto, con su hueco.
pub const ISLA_CELDA_MINIMA: u32 = 112;
pub const ISLA_CELDA_HUECO: u32 = 4;
pub const ISLA_MARGEN: u32 = 12;
/// Las filas de archivos, enlaces y voz.
pub const ISLA_FILA: u32 = 62;
pub const ISLA_FILA_MINIATURA: u32 = 44;
/// La previa de una cosa: las flechas de los lados y el pie con sus botones.
pub const ISLA_FLECHA: u32 = 40;
pub const ISLA_PIE: u32 = 64;
pub const ISLA_BOTON_PIE: u32 = 40;
pub const ISLA_BOTON_PIE_ANCHO: u32 = 150;

/// Donde cae la isla dentro de la ventana, y si se lo queda todo porque la
/// ventana no da ni para la mas estrecha con sus margenes.
pub fn isla_en(ventana: Rect, escala_por_cien: u32) -> (Rect, bool) {
    let e = |v: u32| v * escala_por_cien / 100;
    if ventana.ancho < e(ISLA_ANCHO_MINIMO) + 2 * e(ISLA_MARGEN_X) {
        return (ventana, true);
    }
    let ancho = (ventana.ancho * ISLA_FRACCION / 100)
        .clamp(e(ISLA_ANCHO_MINIMO), e(ISLA_ANCHO_MAXIMO))
        .min(ventana.ancho - 2 * e(ISLA_MARGEN_X));
    let alto = ventana
        .alto
        .saturating_sub(2 * e(ISLA_MARGEN_Y))
        .min(e(ISLA_ALTO_MAXIMO))
        .max(1);
    (
        Rect {
            x: ventana.x + (ventana.ancho as i32 - ancho as i32) / 2,
            y: ventana.y + (ventana.alto as i32 - alto as i32) / 2,
            ancho,
            alto,
        },
        false,
    )
}

/// Como queda repartida la isla. Una sola cuenta para pintarla y para
/// repartir sus clics (ver `ficha_del_chat::disposicion` en la ventana del
/// chat: hubo un tiempo en que eran dos, y los botones no respondian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Isla {
    pub panel: Rect,
    pub cabecera: Rect,
    /// El avatar del chat; en la previa de una cosa, la flecha de volver.
    pub avatar: Rect,
    /// Donde va el nombre y, debajo, lo que tiene.
    pub titulo: Rect,
    pub buscar: Rect,
    pub cerrar: Rect,
    /// La caja de escribir lo que se busca, en el sitio del titulo.
    pub caja_buscar: Rect,
    /// El selector de pestanas (ya con su aire alrededor).
    pub tira: Rect,
    pub contenido: Rect,
    /// La ventana no daba para la isla y la ocupa entera.
    pub pantalla_completa: bool,
}

impl Isla {
    pub fn calcular(caja: Rect, pantalla_completa: bool, escala_por_cien: u32) -> Isla {
        let e = |v: u32| v * escala_por_cien / 100;
        let cabecera = Rect {
            alto: e(ISLA_CABECERA).min(caja.alto),
            ..caja
        };
        let medio = |lado: u32| cabecera.y + (cabecera.alto as i32 - lado as i32) / 2;
        let boton = e(ISLA_BOTON).min(cabecera.alto);
        let cerrar = Rect {
            x: cabecera.derecha() - e(10) as i32 - boton as i32,
            y: medio(boton),
            ancho: boton,
            alto: boton,
        };
        let buscar = Rect {
            x: cerrar.x - boton as i32,
            ..cerrar
        };
        let lado = e(ISLA_AVATAR).min(cabecera.alto);
        let avatar = Rect {
            x: cabecera.x + e(ISLA_AVATAR_X) as i32,
            y: medio(lado),
            ancho: lado,
            alto: lado,
        };
        let tx = avatar.derecha() + e(12) as i32;
        let titulo = Rect {
            x: tx,
            y: cabecera.y,
            ancho: (buscar.x - e(8) as i32 - tx).max(0) as u32,
            alto: cabecera.alto,
        };
        let alto_caja = e(BUSCAR_ALTO).min(cabecera.alto);
        let caja_buscar = Rect {
            x: titulo.x,
            y: medio(alto_caja),
            ancho: titulo.ancho,
            alto: alto_caja,
        };
        let franja = e(ISLA_TIRA).min(caja.alto.saturating_sub(cabecera.alto));
        let tira = Rect {
            x: caja.x + e(ISLA_TIRA_MARGEN_X) as i32,
            y: cabecera.abajo() + e(ISLA_TIRA_MARGEN_Y) as i32,
            ancho: caja.ancho.saturating_sub(2 * e(ISLA_TIRA_MARGEN_X)),
            alto: franja.saturating_sub(2 * e(ISLA_TIRA_MARGEN_Y)),
        };
        let arriba = cabecera.abajo() + franja as i32;
        Isla {
            panel: caja,
            cabecera,
            avatar,
            titulo,
            buscar,
            cerrar,
            caja_buscar,
            tira,
            contenido: Rect {
                x: caja.x,
                y: arriba,
                ancho: caja.ancho,
                alto: (caja.abajo() - arriba).max(0) as u32,
            },
            pantalla_completa,
        }
    }

    /// Donde cae cada pestana en la tira, en el orden de los anchos de texto
    /// (que mide quien pinta). Se reparten centradas con su relleno; si no
    /// caben, primero se les quita relleno y, si aun asi no caben, se
    /// reparte la tira a partes iguales: nunca se sale ninguna.
    pub fn pestanas(&self, anchos_de_texto: &[f32], escala_por_cien: u32) -> Vec<Rect> {
        let e = |v: u32| (v * escala_por_cien / 100) as f32;
        let n = anchos_de_texto.len();
        if n == 0 || self.tira.ancho == 0 {
            return Vec::new();
        }
        let sitio = self.tira.ancho as f32;
        let texto: f32 = anchos_de_texto.iter().sum();
        let relleno = ((sitio - texto) / (2.0 * n as f32))
            .min(e(ISLA_PESTANA_RELLENO))
            .max(0.0);
        let anchos: Vec<f32> = if relleno >= e(ISLA_PESTANA_RELLENO_MINIMO) {
            anchos_de_texto.iter().map(|w| w + 2.0 * relleno).collect()
        } else {
            vec![sitio / n as f32; n]
        };
        let total: f32 = anchos.iter().sum();
        let mut x = self.tira.x as f32 + ((sitio - total) / 2.0).max(0.0);
        anchos
            .iter()
            .map(|ancho| {
                let r = Rect {
                    x: x.round() as i32,
                    y: self.tira.y,
                    ancho: ancho.round().max(0.0) as u32,
                    alto: self.tira.alto,
                };
                x += ancho;
                r
            })
            .collect()
    }

    /// El fondo del selector: solo tan ancho como sus pestanas.
    pub fn fondo_de_la_tira(&self, pestanas: &[Rect]) -> Rect {
        match (pestanas.first(), pestanas.last()) {
            (Some(a), Some(b)) => Rect {
                x: a.x,
                y: self.tira.y,
                ancho: (b.derecha() - a.x).max(0) as u32,
                alto: self.tira.alto,
            },
            _ => Rect {
                ancho: 0,
                ..self.tira
            },
        }
    }

    /// La pildora de la pestana activa, encogida por los cuatro lados.
    pub fn pildora(&self, pestana: Rect, escala_por_cien: u32) -> Rect {
        let hueco = (PILDORA_HUECO * escala_por_cien / 100) as i32;
        Rect {
            x: pestana.x + hueco,
            y: pestana.y + hueco,
            ancho: (pestana.ancho as i32 - 2 * hueco).max(0) as u32,
            alto: (pestana.alto as i32 - 2 * hueco).max(0) as u32,
        }
    }

    /// Lo que ocupa la previa de una cosa: la tira y el contenido juntos
    /// (en la previa no hay pestanas).
    pub fn hueco_de_la_previa(&self) -> Rect {
        let y = self.cabecera.abajo();
        Rect {
            x: self.panel.x,
            y,
            ancho: self.panel.ancho,
            alto: (self.panel.abajo() - y).max(0) as u32,
        }
    }
}

/// La rejilla de miniaturas de la isla: columnas cuadradas que se adaptan al
/// ancho (mas sitio, mas columnas), centradas.
pub fn rejilla_isla(ancho: u32, escala_por_cien: u32) -> Rejilla {
    rejilla_con(
        ancho,
        ISLA_CELDA_MINIMA,
        ISLA_CELDA_HUECO,
        ISLA_MARGEN,
        escala_por_cien,
    )
}

/// Las filas (archivos, enlaces, voz) de la isla, una debajo de otra: la de
/// numero `n`, con el desplazamiento ya restado.
pub fn fila_isla(n: usize, area: Rect, scroll: i32, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let alto = e(ISLA_FILA).max(1);
    Rect {
        x: area.x + e(ISLA_MARGEN / 2) as i32,
        y: area.y + e(ISLA_MARGEN / 2) as i32 + (n as u32 * alto) as i32 - scroll,
        ancho: area.ancho.saturating_sub(e(ISLA_MARGEN)),
        alto,
    }
}

/// Que filas se ven: `(primera, cuantas)`, con una de mas por arriba y por
/// abajo que asoma al desplazar.
pub fn filas_visibles(
    area: Rect,
    cuantas: usize,
    scroll: i32,
    escala_por_cien: u32,
) -> (usize, usize) {
    let alto = (ISLA_FILA * escala_por_cien / 100).max(1) as i32;
    let primera = (scroll / alto).max(0) as usize;
    let caben = (area.alto as i32 / alto + 2) as usize;
    (
        primera.min(cuantas),
        caben.min(cuantas.saturating_sub(primera)),
    )
}

/// Lo que miden todas las filas juntas, con su aire arriba y abajo.
pub fn alto_de_las_filas(cuantas: usize, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    if cuantas == 0 {
        return 0;
    }
    cuantas as u32 * e(ISLA_FILA) + e(ISLA_MARGEN)
}

/// Lo de dentro de una fila de la isla: la miniatura redonda a la izquierda
/// y dos lineas de texto.
pub fn dentro_de_la_fila(fila: Rect, escala_por_cien: u32) -> FilaArchivo {
    let e = |v: u32| v * escala_por_cien / 100;
    let lado = e(ISLA_FILA_MINIATURA).min(fila.alto);
    let x = fila.x + e(10) as i32;
    let miniatura = Rect {
        x,
        y: fila.y + (fila.alto as i32 - lado as i32) / 2,
        ancho: lado,
        alto: lado,
    };
    let texto_x = miniatura.derecha() + e(12) as i32;
    let derecha = fila.derecha() - e(10) as i32;
    FilaArchivo {
        fila,
        miniatura,
        nombre: Punto {
            x: texto_x,
            y: fila.y + e(12) as i32,
        },
        estado: Punto {
            x: texto_x,
            y: fila.y + e(34) as i32,
        },
        ancho_texto: (derecha - texto_x).max(0) as u32,
    }
}

/// La previa de una cosa dentro de la isla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Previa {
    /// Donde se ensena la cosa en grande (la foto, el dibujo, su ficha).
    pub vista: Rect,
    /// Las flechas de la anterior y la siguiente, a los lados.
    pub anterior: Rect,
    pub siguiente: Rect,
    /// El pie, con sus dos botones: abrir y verla en el chat.
    pub pie: Rect,
    pub abrir: Rect,
    pub en_el_chat: Rect,
}

/// Reparte el hueco de la previa (`Isla::hueco_de_la_previa`).
pub fn previa(hueco: Rect, escala_por_cien: u32) -> Previa {
    let e = |v: u32| v * escala_por_cien / 100;
    let alto_pie = e(ISLA_PIE).min(hueco.alto);
    let pie = Rect {
        y: hueco.abajo() - alto_pie as i32,
        alto: alto_pie,
        ..hueco
    };
    let margen = e(ISLA_MARGEN) as i32;
    let flecha = e(ISLA_FLECHA);
    let vista = Rect {
        x: hueco.x + margen + flecha as i32,
        y: hueco.y + margen,
        ancho: (hueco.ancho as i32 - 2 * (margen + flecha as i32)).max(0) as u32,
        alto: (pie.y - hueco.y - 2 * margen).max(0) as u32,
    };
    let medio_y = vista.y + (vista.alto as i32 - flecha as i32) / 2;
    let anterior = Rect {
        x: hueco.x + margen / 2,
        y: medio_y,
        ancho: flecha,
        alto: flecha,
    };
    let siguiente = Rect {
        x: hueco.derecha() - margen / 2 - flecha as i32,
        ..anterior
    };
    let alto_boton = e(ISLA_BOTON_PIE).min(alto_pie);
    let ancho_boton =
        e(ISLA_BOTON_PIE_ANCHO).min(hueco.ancho.saturating_sub(3 * margen as u32) / 2);
    let y_boton = pie.y + (alto_pie as i32 - alto_boton as i32) / 2;
    let juntos = 2 * ancho_boton as i32 + margen;
    let x0 = pie.x + (pie.ancho as i32 - juntos) / 2;
    Previa {
        vista,
        anterior,
        siguiente,
        pie,
        en_el_chat: Rect {
            x: x0,
            y: y_boton,
            ancho: ancho_boton,
            alto: alto_boton,
        },
        abrir: Rect {
            x: x0 + ancho_boton as i32 + margen,
            y: y_boton,
            ancho: ancho_boton,
            alto: alto_boton,
        },
    }
}

/// Donde cabe una imagen de `ancho` x `alto` dentro de `caja` sin recortarla
/// ni deformarla, centrada (la previa de una foto; la rejilla recorta).
pub fn encajar(ancho: u32, alto: u32, caja: Rect) -> Rect {
    if ancho == 0 || alto == 0 || caja.ancho == 0 || caja.alto == 0 {
        return Rect {
            ancho: 0,
            alto: 0,
            ..caja
        };
    }
    let factor = (caja.ancho as f64 / ancho as f64).min(caja.alto as f64 / alto as f64);
    let w = ((ancho as f64 * factor).round() as u32).clamp(1, caja.ancho);
    let h = ((alto as f64 * factor).round() as u32).clamp(1, caja.alto);
    Rect {
        x: caja.x + (caja.ancho as i32 - w as i32) / 2,
        y: caja.y + (caja.alto as i32 - h as i32) / 2,
        ancho: w,
        alto: h,
    }
}

// --- Las fotos de las notas ricas en la ficha (25-oct-2026) -----------------
//
// Una nota rica lleva sus fotos dentro del texto (`![…](pixpin:files/…)`,
// ver `nota_rica` y `docs/nota-rica-del-chat.md`). En «Fotos» sale CADA UNA
// como una miniatura mas, en el sitio de su nota: quien busca la foto de la
// obra no piensa en si la mando sola o con texto.

/// Una cosa de la ficha: un mensaje entero o una foto de una nota rica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cosa {
    /// Su posicion en los mensajes del chat.
    pub mensaje: usize,
    /// En una nota rica, que foto suya es (en el orden de
    /// `nota_rica::imagenes`, el mismo que cuenta la burbuja); `None`, el
    /// mensaje entero.
    pub foto: Option<usize>,
}

impl Cosa {
    /// El mensaje entero.
    pub fn entera(mensaje: usize) -> Cosa {
        Cosa {
            mensaje,
            foto: None,
        }
    }
}

/// Las fotos de una nota rica, en el orden en que se ven en su burbuja. Lo
/// que no es una foto (un PDF, un audio) no cuenta. Sin `![` no se parte el
/// texto: la ficha pregunta por cada nota en cada fotograma.
pub fn fotos_de_la_nota(texto: &str) -> Vec<crate::nota_rica::Imagen> {
    if !texto.contains("![") {
        return Vec::new();
    }
    let bloques = crate::nota_rica::bloques(texto.trim());
    crate::nota_rica::imagenes(&bloques).cloned().collect()
}

/// **Lo que sale en «Fotos»**: los mensajes de foto (`fotos`) y, de cada
/// nota (`notas`), cada una de sus fotos, todo en el orden del chat. Las dos
/// listas son posiciones en los mensajes, de menor a mayor; `cuantas_fotos`
/// dice cuantas lleva la nota de esa posicion. Una nota sin fotos no sale.
pub fn con_las_fotos_de_las_notas(
    fotos: &[usize],
    notas: &[usize],
    cuantas_fotos: impl Fn(usize) -> usize,
) -> Vec<Cosa> {
    let mut salida = Vec::with_capacity(fotos.len());
    let (mut f, mut n) = (fotos.iter().peekable(), notas.iter().peekable());
    loop {
        match (f.peek(), n.peek()) {
            (Some(&&a), Some(&&b)) if a <= b => {
                salida.push(Cosa::entera(a));
                f.next();
            }
            (_, Some(&&b)) => {
                salida.extend((0..cuantas_fotos(b)).map(|k| Cosa {
                    mensaje: b,
                    foto: Some(k),
                }));
                n.next();
            }
            (Some(&&a), None) => {
                salida.push(Cosa::entera(a));
                f.next();
            }
            (None, None) => return salida,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ventana() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 768,
        }
    }

    #[test]
    fn la_isla_crece_con_la_ventana_sin_pasarse_y_se_centra() {
        let (normal, completa) = isla_en(ventana(), 100);
        assert!(!completa);
        assert_eq!(normal.ancho, 1024 * ISLA_FRACCION / 100);
        let sobra = normal.x - (ventana().derecha() - normal.derecha());
        assert!(sobra.abs() <= 1, "centrada: {sobra}");
        assert_eq!(normal.y, ventana().abajo() - normal.abajo());
        // En un monitor enorme se queda en su maximo, ancho y alto.
        let enorme = Rect {
            x: 0,
            y: 0,
            ancho: 3000,
            alto: 2000,
        };
        let (grande, _) = isla_en(enorme, 100);
        assert_eq!(grande.ancho, ISLA_ANCHO_MAXIMO);
        assert_eq!(grande.alto, ISLA_ALTO_MAXIMO);
        // Y al 200 % mide el doble de minimo.
        let (doble, _) = isla_en(enorme, 200);
        assert!(doble.ancho >= ISLA_ANCHO_MINIMO * 2);
        // Caso negativo: en una ventana pequena se lo queda todo.
        let pequena = Rect {
            x: 0,
            y: 0,
            ancho: ISLA_ANCHO_MINIMO + 2 * ISLA_MARGEN_X - 1,
            alto: 500,
        };
        assert_eq!(isla_en(pequena, 100), (pequena, true));
    }

    #[test]
    fn las_piezas_de_la_isla_no_se_pisan() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        assert_eq!(i.cerrar.derecha() + 10, i.cabecera.derecha());
        assert_eq!(i.buscar.derecha(), i.cerrar.x, "la lupa junto al aspa");
        assert!(i.avatar.derecha() < i.titulo.x);
        assert!(i.titulo.derecha() < i.buscar.x, "el nombre no pisa la lupa");
        assert_eq!(i.caja_buscar.x, i.titulo.x);
        assert!(i.tira.y > i.cabecera.abajo() - 1 && i.tira.abajo() < i.contenido.y + 1);
        assert_eq!(i.contenido.abajo(), caja.abajo());
        // La previa usa la tira y el contenido juntos.
        let h = i.hueco_de_la_previa();
        assert_eq!((h.y, h.abajo()), (i.cabecera.abajo(), caja.abajo()));
    }

    #[test]
    fn las_pestanas_de_la_isla_se_centran_y_nunca_se_salen() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        let pocas = i.pestanas(&[40.0, 60.0], 100);
        assert_eq!(pocas[0].ancho, 40 + 2 * ISLA_PESTANA_RELLENO);
        assert_eq!(pocas[1].x, pocas[0].derecha(), "pegadas");
        // Centradas en la tira.
        let fondo = i.fondo_de_la_tira(&pocas);
        let izq = fondo.x - i.tira.x;
        let der = i.tira.derecha() - fondo.derecha();
        assert!((izq - der).abs() <= 1, "{izq} {der}");
        // Muchas: menos relleno, y si aun asi no caben, a partes iguales.
        let muchas = i.pestanas(&[70.0; 10], 100);
        assert_eq!(muchas.len(), 10);
        assert!(muchas[9].derecha() <= i.tira.derecha() + 1);
        assert!(muchas[0].x >= i.tira.x);
        // La pildora, dentro de su pestana.
        let p = i.pildora(pocas[0], 100);
        assert!(p.x > pocas[0].x && p.derecha() < pocas[0].derecha());
        // Caso negativo: sin pestanas no hay fondo.
        assert_eq!(i.fondo_de_la_tira(&[]).ancho, 0);
    }

    #[test]
    fn la_rejilla_de_la_isla_pone_mas_columnas_cuanto_mas_ancha() {
        let estrecha = rejilla_isla(380, 100);
        let ancha = rejilla_isla(720, 100);
        assert!(ancha.columnas > estrecha.columnas);
        for r in [estrecha, ancha] {
            assert!(r.lado >= ISLA_CELDA_MINIMA);
            assert_eq!(r.hueco, ISLA_CELDA_HUECO);
        }
        // Las celdas, con su hueco de la isla y no el del panel viejo.
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 720,
            alto: 500,
        };
        let a = ancha.celda(0, area, 0);
        let b = ancha.celda(1, area, 0);
        assert_eq!(b.x - a.derecha(), ISLA_CELDA_HUECO as i32);
        // La de la fila siguiente, debajo y en la primera columna.
        let c = ancha.celda(ancha.columnas as usize, area, 0);
        assert_eq!((c.x, c.y), (a.x, a.abajo() + ISLA_CELDA_HUECO as i32));
    }

    #[test]
    fn las_filas_de_la_isla_van_una_bajo_otra_y_solo_se_miran_las_que_se_ven() {
        let area = Rect {
            x: 100,
            y: 200,
            ancho: 500,
            alto: 310,
        };
        let a = fila_isla(0, area, 0, 100);
        let b = fila_isla(1, area, 0, 100);
        assert_eq!(b.y, a.abajo());
        assert!(a.x > area.x && a.derecha() < area.derecha());
        let (primera, cuantas) = filas_visibles(area, 100, 0, 100);
        assert_eq!(primera, 0);
        assert_eq!(cuantas, 310 / ISLA_FILA as usize + 2);
        let (primera, _) = filas_visibles(area, 100, 3 * ISLA_FILA as i32, 100);
        assert_eq!(primera, 3);
        // Caso negativo: no se pasa de las que hay.
        assert_eq!(filas_visibles(area, 2, 0, 100), (0, 2));
        assert_eq!(alto_de_las_filas(0, 100), 0);
        let f = dentro_de_la_fila(a, 100);
        assert!(f.miniatura.x > a.x && f.miniatura.abajo() <= a.abajo());
        assert!(f.nombre.x > f.miniatura.derecha() && f.estado.y > f.nombre.y);
    }

    #[test]
    fn la_previa_deja_las_flechas_a_los_lados_y_los_botones_abajo() {
        let (caja, completa) = isla_en(ventana(), 100);
        let i = Isla::calcular(caja, completa, 100);
        let h = i.hueco_de_la_previa();
        let p = previa(h, 100);
        assert!(p.anterior.derecha() <= p.vista.x);
        assert!(p.siguiente.x >= p.vista.derecha());
        assert!(p.vista.abajo() <= p.pie.y);
        assert_eq!(p.pie.abajo(), h.abajo());
        assert!(p.en_el_chat.derecha() < p.abrir.x, "sin pisarse");
        assert!(p.abrir.abajo() <= p.pie.abajo() && p.abrir.y >= p.pie.y);
        // Centrados: lo que sobra a cada lado del par es lo mismo.
        let izq = p.en_el_chat.x - p.pie.x;
        let der = p.pie.derecha() - p.abrir.derecha();
        assert!((izq - der).abs() <= 1);
    }

    #[test]
    fn encajar_no_recorta_ni_deforma() {
        let caja = Rect {
            x: 10,
            y: 20,
            ancho: 400,
            alto: 300,
        };
        // Apaisada: llena el ancho y sobra arriba y abajo por igual.
        let r = encajar(800, 400, caja);
        assert_eq!((r.ancho, r.alto), (400, 200));
        assert_eq!(r.y - caja.y, caja.abajo() - r.abajo());
        // Vertical: llena el alto.
        let r = encajar(100, 600, caja);
        assert_eq!(r.alto, 300);
        assert_eq!(r.ancho, 50);
        // Caso negativo: una imagen sin tamano no ocupa nada.
        assert_eq!(encajar(0, 10, caja).ancho, 0);
    }

    #[test]
    fn la_cuadricula_reparte_el_sobrante_y_queda_centrada() {
        let r = rejilla_isla(500, 100);
        assert!(r.lado >= ISLA_CELDA_MINIMA, "ninguna por debajo del minimo");
        // Lo que ocupa, mas los dos margenes, es el ancho entero (o uno menos
        // por el redondeo): no sobra un pico a la derecha.
        let ocupa = r.columnas * r.lado + (r.columnas - 1) * r.hueco;
        assert!(500 - (ocupa + 2 * r.margen) <= 1, "sobra demasiado");
        // Caso negativo: en un ancho estrechisimo queda una columna, no cero.
        let r = rejilla_isla(60, 100);
        assert_eq!(r.columnas, 1);
        assert!(r.lado >= 1);
    }

    #[test]
    fn de_mil_fotos_solo_se_pintan_las_que_entran() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 600,
            alto: 500,
        };
        let r = rejilla_isla(area.ancho, 100);
        let (primera, cuantas) = r.visibles(area, 1000, 0);
        assert_eq!(primera, 0);
        assert!(cuantas < 60, "demasiadas: {cuantas}");
        // Bajando, se empieza por otra fila entera.
        let paso = (r.lado + r.hueco) as i32;
        let (primera, _) = r.visibles(area, 1000, paso * 3);
        assert_eq!(primera, 3 * r.columnas as usize, "filas enteras");
        // Sin fotos no ocupa nada; una sola, una fila.
        assert_eq!(r.alto_total(0), 0);
        assert_eq!(r.visibles(area, 0, 0), (0, 0));
        assert_eq!(r.alto_total(1), r.lado);
    }

    #[test]
    fn las_fotos_de_una_nota_salen_cada_una_en_el_sitio_de_su_nota() {
        // Mensajes: 0 foto, 1 nota con dos fotos, 2 nota sin fotos, 3 foto,
        // 4 nota con una.
        let fotos = [0, 3];
        let notas = [1, 2, 4];
        let cuantas = |n: usize| match n {
            1 => 2,
            4 => 1,
            _ => 0,
        };
        let c = con_las_fotos_de_las_notas(&fotos, &notas, cuantas);
        let nota = |mensaje, k| Cosa {
            mensaje,
            foto: Some(k),
        };
        assert_eq!(
            c,
            vec![
                Cosa::entera(0),
                nota(1, 0),
                nota(1, 1),
                Cosa::entera(3),
                nota(4, 0)
            ]
        );
        // Caso negativo: sin notas, las fotos tal cual; sin nada, nada.
        assert_eq!(
            con_las_fotos_de_las_notas(&[5, 7], &[], cuantas),
            vec![Cosa::entera(5), Cosa::entera(7)]
        );
        assert!(con_las_fotos_de_las_notas(&[], &[2], cuantas).is_empty());
    }

    #[test]
    fn de_una_nota_solo_cuentan_sus_fotos_y_en_el_orden_de_la_burbuja() {
        let nota = "Visita\n![](pixpin:files/guardados/pc/c/notas/1-a.png)\n![planta|300](pixpin:files/guardados/pc/c/notas/2-b.jpg)\n\nY el plano\n![Plano.pdf](pixpin:files/guardados/pc/c/notas/3-Plano.pdf)\n![](notas/4-c.webp)";
        let f = fotos_de_la_nota(nota);
        let enlaces: Vec<&str> = f.iter().map(|i| i.enlace.as_str()).collect();
        assert_eq!(
            enlaces,
            [
                "pixpin:files/guardados/pc/c/notas/1-a.png",
                "pixpin:files/guardados/pc/c/notas/2-b.jpg",
                "notas/4-c.webp"
            ]
        );
        // El mismo orden en que la burbuja las cuenta para abrirlas.
        let b = crate::nota_rica::bloques(nota.trim());
        let de_la_burbuja: Vec<_> = crate::nota_rica::imagenes(&b).cloned().collect();
        assert_eq!(f, de_la_burbuja);
        // Caso negativo: una nota llana no tiene fotos.
        assert!(fotos_de_la_nota("hola, mira https://x.com/a.png").is_empty());
    }
}
