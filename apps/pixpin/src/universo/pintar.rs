//! Pintar el universo: fondo, astros, lineas y nebulosas (D217-D219).
//!
//! Todo se pinta en pixeles de pantalla, con la vista del mundo quitada: el
//! nivel de detalle ya decide que se ve a cada zoom, y un rotulo que
//! escalara con la camara seria ilegible lejos y enorme cerca. El nucleo
//! (`pixpin-universo`) dice QUE se ve; aqui solo se decide COMO.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_motor2d::{Camara, Punto2};
use pixpin_render::icono::{Icono, Pintura, TrazoIcono};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_universo::ficha::{ClaseLuna, FichaLuna};
use pixpin_universo::{Astro, Clase, IdAstro, Nivel, TipoConexion, Visto, nebulosa};

use super::cielo::Cielo;
use super::estrellas::Estrellas;
use crate::caja_dibujo::hex;
use crate::miniaturas::Miniaturas;

/// Los colores del universo: el **tema Cosmos del movil**, medido en
/// `ui/theme/Cosmos.kt` (ver `docs/investigacion/2026-09-19-universo-android.md`).
/// Es oscuro siempre, sea cual sea el tema de Windows: es la metafora, y en
/// una pantalla OLED ahorra bateria.
pub struct Paleta {
    pub espacio: Color,
    pub texto: Color,
    pub texto_suave: Color,
    pub acento: Color,
    pub peligro: Color,
    pub panel: Color,
    pub borde_panel: Color,
}

pub const PALETA: Paleta = Paleta {
    // `CosmosBase`.
    espacio: hex(0x0B0F24),
    // `CosmosTexto` y `CosmosTextoSuave`: el suave de antes (#8A93A8) era
    // mucho mas apagado que el del movil y hacia ilegibles los segundos
    // rotulos.
    texto: hex(0xE8EAF6),
    texto_suave: hex(0xB4B9D6),
    // `CosmosDorado`: el acento del movil es dorado, no azul.
    acento: hex(0xFFD27A),
    peligro: hex(0xFFB4AB),
    // `surfaceContainerHighest` con cielo detras: vidrio azul translucido.
    panel: Color {
        r: 0x1C as f32 / 255.0,
        g: 0x23 as f32 / 255.0,
        b: 0x50 as f32 / 255.0,
        a: 0.76,
    },
    // `outlineVariant`.
    borde_panel: hex(0x343C6A),
};

/// Los ocho colores de sol del movil (`Galaxia.kt:928-931`).
const COLORES_DE_SOL: [u32; 8] = [
    0xFFB84D, 0xFF7A6B, 0x7C9CFF, 0x5FE0C2, 0xB48CFF, 0x6FD3FF, 0xFF8FB1, 0xFFE066,
];

/// El `hashCode` de `String` de Java, que es con el que el movil elige el
/// color de un sol. Se copia tal cual —con su desbordamiento— para que **el
/// mismo proyecto salga del mismo color en los dos aparatos**; cualquier otra
/// funcion daria colores distintos y el usuario dejaria de reconocerlos.
fn hash_java(s: &str) -> i32 {
    s.chars()
        .flat_map(|c| {
            let mut u = [0u16; 2];
            c.encode_utf16(&mut u).to_vec()
        })
        .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(u as i32))
}

/// El color de un sol (nuestra galaxia), como en el movil: fijo por proyecto.
///
/// No es `color_avatar` a proposito: en el movil el sol tampoco lleva el
/// color del avatar del chat (`colorDelSol` es solo del universo), asi que
/// copiar el del chat nos alejaria de el en vez de acercarnos.
pub fn color_de_sol(proyecto: &str) -> Color {
    let i = hash_java(proyecto).rem_euclid(COLORES_DE_SOL.len() as i32) as usize;
    hex(COLORES_DE_SOL[i])
}

/// `lerp` entre dos colores, como el `lerp` de Compose: mezcla lineal de los
/// canales.
pub fn mezcla(a: Color, b: Color, t: f32) -> Color {
    let m = |x: f32, y: f32| x + (y - x) * t;
    Color {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: m(a.a, b.a),
    }
}

/// Lo que se atenua lo que no esta enfocado (D225).
const FUERA_DE_FOCO: f32 = 0.35;

/// Una luna cuyo fichero no esta en este equipo va al 40 % (D219): se ve
/// que existe, y se ve que no se puede abrir.
pub fn opacidad_de_luna(en_equipo: bool) -> f32 {
    if en_equipo { 1.0 } else { 0.4 }
}

pub fn con_alfa(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

// --- Lo que se comparte con el chat -----------------------------------------
//
// La extension y el tamano son los MISMOS que en el chat, y se toman de
// alli: el usuario reconoce un archivo por su chapa y por su peso, y si las
// dos pantallas discreparan en eso dejaria de fiarse de las dos.
//
// El COLOR de una galaxia ya no: es el del sol del movil (`color_de_sol`),
// porque alli tampoco es el del avatar del chat.

pub(crate) use crate::ventana_chat::extension_de;

/// El tamano como lo escribe el chat en su cabecera (y el movil).
pub fn tamano_legible(bytes: i64) -> String {
    pixpin_ui::chat::tamano_corto(bytes.max(0) as u64)
}

/// El color de la chapa de un archivo segun su tipo. El chat no pinta
/// chapas de color (su fila lleva el icono), asi que esta es solo del
/// universo.
pub fn color_de_extension(extension: &str) -> Color {
    match extension {
        "pdf" => hex(0xd93b3b),
        "apk" => hex(0x3fa34d),
        "zip" | "rar" | "7z" => hex(0xd2a022),
        "doc" | "docx" | "rtf" | "txt" | "md" => hex(0x2f6feb),
        "xls" | "xlsx" | "csv" => hex(0x1a7f37),
        "ppt" | "pptx" => hex(0xd1481f),
        "html" | "htm" => hex(0x4184c4),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => hex(0x8250df),
        "mp4" | "mov" | "mkv" | "webm" => hex(0x6e40c9),
        "mp3" | "wav" | "m4a" | "ogg" => hex(0xbf3989),
        "dib" => hex(0x2a9d8f),
        _ => hex(0x57606a),
    }
}

// --- Iconos -----------------------------------------------------------------

const fn trazo(d: &'static str) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Nada,
        trazo: Pintura::Actual,
        grosor: 1.75,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }
}

/// «planet» de Tabler Icons (MIT, citado en THIRD-PARTY-NOTICES.md).
pub const PLANETA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[
        trazo(
            "M18.816 13.58c2.292 2.138 3.546 4 3.092 4.9c-.745 1.46 -5.783 -.259 -11.255 -3.838c-5.47 -3.579 -9.304 -7.664 -8.56 -9.123c.464 -.91 2.926 -.444 5.803 .805",
        ),
        trazo("M12 12m-7 0a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"),
    ],
};

/// «mood-smile» de Tabler Icons (MIT).
pub const CARITA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[
        trazo("M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0"),
        trazo("M9 10l.01 0"),
        trazo("M15 10l.01 0"),
        trazo("M9.5 15a3.5 3.5 0 0 0 5 0"),
    ],
};

/// «line» de Tabler Icons (MIT).
pub const LINEA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[
        trazo("M6 18m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0"),
        trazo("M18 6m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0"),
        trazo("M7.5 16.5l9 -9"),
    ],
};

/// El icono de cada clase de luna (D219). `None` para el archivo, que se
/// ensena con la chapa de su extension y no con un dibujo.
pub fn icono_de_clase(c: ClaseLuna) -> Option<&'static Icono> {
    use pixpin_render::iconos_excalidraw as i;
    match c {
        ClaseLuna::Imagen => Some(&i::IMAGE_ICON),
        ClaseLuna::Archivo => None,
        ClaseLuna::Voz => Some(&i::MICROPHONE_ICON),
        ClaseLuna::Dibujo => Some(&i::PENCIL_ICON),
        ClaseLuna::Pagina => Some(&i::FILE),
        ClaseLuna::MiniApp => Some(&i::GRID_ICON),
        ClaseLuna::Proyecto => Some(&PLANETA),
        ClaseLuna::Nota => Some(&i::STICKY_NOTE_TOOL_ICON),
    }
}

// --- Lo que se pinta --------------------------------------------------------

/// Todo lo que hace falta para pintar un fotograma. Prestado: pintar no
/// cambia nada.
pub struct Contexto<'a> {
    pub u: &'a pixpin_universo::Universo,
    pub vistos: &'a [Visto],
    /// Las fichas cargadas, por `codigo`.
    pub fichas: &'a HashMap<String, FichaLuna>,
    /// Proyecto -> nombre, del indice.
    pub nombres: &'a HashMap<String, String>,
    /// Proyecto -> cuantas lunas tiene, para el rotulo de la galaxia.
    pub cuentas: &'a HashMap<String, usize>,
    pub seleccion: &'a [IdAstro],
    pub enfocado: Option<IdAstro>,
    /// `(id, fase 0..1)`: el anillo rojo que se abre y se apaga.
    pub destellos: &'a [(IdAstro, f32)],
    pub ligero: bool,
    /// Pixeles fisicos por pixel logico.
    pub escala: f32,
    pub miniaturas: &'a Miniaturas,
    /// La raiz de los proyectos, para encontrar el fichero de una foto.
    pub raiz: &'a Path,
    /// «N archivos», ya en el idioma del usuario: el segundo rotulo de un
    /// sol, como en el movil.
    pub etiqueta_archivos: &'a dyn Fn(usize) -> String,
    /// La isla de herramientas, en pixeles de pantalla: un rotulo no se
    /// pinta debajo de ella.
    pub estorbo: RectF,
    /// Lo mas arriba que puede llegar un rotulo: el pie de la barra de ruta.
    pub techo: f32,
}

impl Contexto<'_> {
    /// Si `id` es el enfocado o esta dentro de el. Sin enfoque, todo lo esta.
    fn en_foco(&self, id: IdAstro) -> bool {
        let Some(f) = self.enfocado else { return true };
        let mut actual = Some(id);
        while let Some(i) = actual {
            if i == f {
                return true;
            }
            actual = self.u.astro(i).and_then(|a| a.padre);
        }
        false
    }

    fn ficha_de(&self, a: &Astro) -> Option<&FichaLuna> {
        a.codigo().and_then(|c| self.fichas.get(c))
    }

    fn nombre_de(&self, a: &Astro) -> String {
        match &a.clase {
            Clase::Galaxia { proyecto } => self
                .nombres
                .get(proyecto)
                .cloned()
                .unwrap_or_else(|| proyecto.clone()),
            Clase::Planeta => a.nombre.clone(),
            Clase::Luna { .. } => self
                .ficha_de(a)
                .map(|f| f.nombre.clone())
                .unwrap_or_default(),
        }
    }
}

/// La ruta en disco de la foto de una ficha, si esta aqui.
pub fn ruta_de_foto(raiz: &Path, f: &FichaLuna) -> Option<PathBuf> {
    if f.clase != ClaseLuna::Imagen || !f.en_equipo {
        return None;
    }
    let r = f.ruta.as_deref()?;
    Some(pixpin_proyecto::almacen::carpeta(raiz, &f.proyecto).join(r))
}

fn a_pantalla(camara: &Camara, x: f32, y: f32) -> (f32, f32) {
    let q = camara.a_pantalla(Punto2::nuevo(x, y));
    (q.x, q.y)
}

/// Un texto centrado en `x`, con su parte de arriba en `y`.
fn texto_centrado(p: &Pintor, texto: &str, x: f32, y: f32, tam: f32, color: Color) {
    let (w, _) = p.medir_texto(texto, tam);
    p.texto(texto, x - w / 2.0, y, tam, color);
}

/// Si dos cajas se tocan.
fn chocan(a: RectF, b: RectF) -> bool {
    a.x < b.x + b.ancho && b.x < a.x + a.ancho && a.y < b.y + b.alto && b.y < a.y + a.alto
}

/// **Donde cae el rotulo de un astro sin que lo tape un panel** (la falla que
/// vio el usuario: el nombre de la galaxia detras de la isla de
/// herramientas).
///
/// Los paneles se pintan DESPUES que los astros —van encima de las
/// anotaciones—, asi que un rotulo que caiga bajo uno desaparece. En vez de
/// cambiar el orden (las anotaciones tienen que quedarse entre medias), el
/// rotulo se aparta: si donde preferiria ir toca la isla o se sale por
/// encima del techo, se va al otro lado del astro; si alli tampoco cabe, se
/// queda pegado al techo.
pub fn y_de_rotulo(
    x: f32,
    ancho: f32,
    alto: f32,
    preferida: f32,
    alternativa: f32,
    estorbo: RectF,
    techo: f32,
) -> f32 {
    let caja = |y: f32| RectF {
        x: x - ancho / 2.0,
        y,
        ancho,
        alto,
    };
    let libre = |y: f32| y >= techo && !chocan(caja(y), estorbo);
    if libre(preferida) {
        preferida
    } else if libre(alternativa) {
        alternativa
    } else {
        preferida.max(techo)
    }
}

/// El cielo del movil: el degradado y las dos nebulosas, ya horneados en un
/// bitmap opaco (ver `super::cielo`), y encima las estrellas.
///
/// `rapido`: la camara se esta moviendo, solo la capa lejana de estrellas
/// (ver `Sesion::en_movimiento`). El cielo se pinta siempre: es lo que da el
/// ambiente, y quitarlo al mover haria parpadear el fondo entero.
pub fn fondo(
    p: &Pintor,
    cielo: Option<&Cielo>,
    estrellas: Option<&Estrellas>,
    camara: &Camara,
    rapido: bool,
    ancho: f32,
    alto: f32,
) {
    if !cielo.is_some_and(|c| c.pintar(p, ancho, alto)) {
        // Sin bitmap (dispositivo recien perdido): un cielo liso, que es
        // feo pero no deja la pantalla con basura.
        p.limpiar(PALETA.espacio);
    }
    if let Some(e) = estrellas {
        e.pintar(p, (camara.x * camara.zoom, camara.y * camara.zoom), rapido);
    }
}

/// Los astros, en el orden en que vienen (galaxias, planetas, lunas).
pub fn astros(p: &Pintor, c: &Contexto, camara: &Camara) {
    let e = c.escala;
    for v in c.vistos {
        let Some(a) = c.u.astro(v.id) else { continue };
        let centro = a_pantalla(camara, a.x, a.y);
        // El nivel se decide en pixeles logicos (D218); se pinta en fisicos.
        let r = v.radio_px * c.escala;
        let foco = if c.en_foco(a.id) { 1.0 } else { FUERA_DE_FOCO };
        match &a.clase {
            Clase::Galaxia { proyecto } => {
                let color = color_de_sol(proyecto);
                let nombre = c.nombre_de(a);
                let lunas = c.cuentas.get(proyecto).copied().unwrap_or(0);
                match v.nivel {
                    // Abierta: se le ven las lunas dentro, asi que el cuerpo
                    // se queda en un aro tenue para no taparlas, y el nombre
                    // va arriba, fuera del disco.
                    Nivel::Vista => {
                        p.anillo(centro, r, 1.5 * e, con_alfa(color, 0.15 * foco));
                        rotulo_de_sol(p, c, &nombre, lunas, centro, r, foco, color, false);
                    }
                    Nivel::Disco => {
                        sol(p, centro, r, color, foco, c.ligero, e);
                        if !c.ligero && r >= 14.0 * e {
                            inicial_del_sol(p, &nombre, centro, r, foco);
                        }
                        rotulo_de_sol(p, c, &nombre, lunas, centro, r, foco, color, true);
                    }
                    _ => p.circulo(centro, 2.0 * e, con_alfa(color, foco)),
                }
            }
            Clase::Planeta => {
                let color = a.color.map(hex).unwrap_or(PALETA.acento);
                if v.nivel >= Nivel::Icono {
                    // Abierto: translucido, para que sus lunas se lean
                    // encima, y con el borde de su color.
                    p.circulo(centro, r, con_alfa(color, 0.22 * foco));
                    p.anillo(centro, r, 1.5 * e, con_alfa(color, 0.8 * foco));
                    p.anillo(
                        centro,
                        r * 0.8,
                        1.0 * e,
                        con_alfa(PALETA.texto_suave, 0.4 * foco),
                    );
                    if let Some(emoji) = &a.emoji {
                        let tam = r * 0.6;
                        let (w, h) = p.medir_texto(emoji, tam);
                        p.texto_color(
                            emoji,
                            centro.0 - w / 2.0,
                            centro.1 - h / 2.0,
                            tam,
                            con_alfa(PALETA.texto, foco),
                        );
                    }
                    texto_centrado(
                        p,
                        &a.nombre,
                        centro.0,
                        centro.1 + r + 4.0 * e,
                        13.0 * e,
                        con_alfa(PALETA.texto, foco),
                    );
                } else {
                    p.circulo(centro, r, con_alfa(color, foco));
                }
            }
            Clase::Luna { .. } => luna(p, c, a, v, centro, foco),
        }
        if c.seleccion.contains(&a.id) {
            p.anillo(centro, r + 3.0 * e, 2.0 * e, PALETA.acento);
        }
    }
    for (id, fase) in c.destellos {
        let Some(a) = c.u.astro(*id) else { continue };
        let centro = a_pantalla(camara, a.x, a.y);
        let r = (a.radio * camara.zoom).max(8.0 * e);
        p.anillo(
            centro,
            r * (1.0 + 0.3 * fase),
            2.0 * e,
            con_alfa(PALETA.peligro, 1.0 - fase),
        );
    }
}

/// **El sol de un proyecto, como en el movil** (`Galaxia.kt:985-1038`): dos
/// coronas, el cuerpo en degradado y el borde claro.
///
/// Todo con `Pintor::brillo` —un bitmap de 256 px cacheado por color— y no
/// con pinceles radiales: un pincel por sol y fotograma costaba 0,5 ms de
/// CPU, once milisegundos con veinte soles (medido, `lienzo.rs:529-535`).
///
/// En Ligero se queda en un disco liso: la grafica integrada del usuario no
/// tiene los cinco blits por sol.
fn sol(p: &Pintor, centro: (f32, f32), r: f32, color: Color, foco: f32, ligero: bool, e: f32) {
    if ligero {
        p.circulo(centro, r, con_alfa(color, 0.35 * foco));
        p.anillo(centro, r, 1.5 * e, con_alfa(color, 0.7 * foco));
        return;
    }
    // La corona ancha del color, y la calida pegada al cuerpo.
    p.brillo(centro, r * 2.1, color, 0.40 * foco);
    p.brillo(centro, r * 1.3, color, 0.25 * foco);
    p.brillo(centro, r * 1.3, Color::BLANCO, 0.35 * foco);
    // El cuerpo: oscuro en el borde y claro en el centro, que es como se ve
    // una bola iluminada desde dentro.
    p.circulo(centro, r, con_alfa(mezcla(color, Color::NEGRO, 0.45), foco));
    p.brillo(centro, r, mezcla(color, Color::BLANCO, 0.35), foco);
    p.anillo(
        centro,
        r,
        (2.0 * e).min(r * 0.25),
        con_alfa(mezcla(color, Color::BLANCO, 0.5), 0.95 * foco),
    );
}

/// La inicial dentro del sol, como el movil cuando el proyecto no tiene
/// portada: en blanco, a 0,38 del diametro.
fn inicial_del_sol(p: &Pintor, nombre: &str, centro: (f32, f32), r: f32, foco: f32) {
    let Some(letra) = nombre
        .trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
    else {
        return;
    };
    let tam = r * 2.0 * 0.38;
    let (w, h) = p.medir_texto(&letra, tam);
    p.texto(
        &letra,
        centro.0 - w / 2.0,
        centro.1 - h / 2.0,
        tam,
        con_alfa(Color::BLANCO, 0.95 * foco),
    );
}

/// **El rotulo de un sol, en dos lineas** como en el movil: el nombre, y
/// debajo «N archivos» mas apagado. Antes iba todo en una, «Nombre · 12»,
/// que se leia como parte del nombre.
///
/// `debajo`: si el sitio preferido es bajo el disco (sol cerrado) o encima
/// (galaxia abierta, donde debajo estarian sus lunas).
#[allow(clippy::too_many_arguments)]
fn rotulo_de_sol(
    p: &Pintor,
    c: &Contexto,
    nombre: &str,
    lunas: usize,
    centro: (f32, f32),
    r: f32,
    foco: f32,
    color: Color,
    debajo: bool,
) {
    let e = c.escala;
    let tam = 14.0 * e;
    let tam_bajo = 11.5 * e;
    let (w, h) = p.medir_texto(nombre, tam);
    let cuenta = (lunas > 0).then(|| (c.etiqueta_archivos)(lunas));
    let alto = h + cuenta.as_ref().map_or(0.0, |_| tam_bajo * 1.25);
    let ancho = w.max(
        cuenta
            .as_ref()
            .map_or(0.0, |t| p.medir_texto(t, tam_bajo).0),
    );
    let y = y_de_rotulo(
        centro.0,
        ancho,
        alto,
        if debajo {
            centro.1 + r + 8.0 * e
        } else {
            centro.1 - r - alto - 10.0 * e
        },
        if debajo {
            centro.1 - r - alto - 10.0 * e
        } else {
            centro.1 + r + 8.0 * e
        },
        c.estorbo,
        c.techo,
    );
    texto_centrado(
        p,
        nombre,
        centro.0,
        y,
        tam,
        con_alfa(PALETA.texto, 0.96 * foco),
    );
    if let Some(t) = cuenta {
        texto_centrado(
            p,
            &t,
            centro.0,
            y + h + 1.0 * e,
            tam_bajo,
            con_alfa(mezcla(PALETA.texto_suave, color, 0.35), 0.75 * foco),
        );
    }
}

/// La caja de pantalla de una luna: el cuadrado que la contiene.
fn caja_de(centro: (f32, f32), r: f32) -> RectF {
    RectF {
        x: centro.0 - r,
        y: centro.1 - r,
        ancho: 2.0 * r,
        alto: 2.0 * r,
    }
}

/// El icono de una luna dentro de `caja`: el de su clase, o la chapa de su
/// extension para un archivo.
pub fn icono_de_luna(p: &Pintor, f: &FichaLuna, caja: RectF, alfa: f32, e: f32) {
    match icono_de_clase(f.clase) {
        Some(i) => p.icono(i, caja, con_alfa(PALETA.texto, alfa)),
        None => {
            let ext = extension_de(&f.nombre);
            p.rellenar_redondeado(
                caja,
                caja.ancho * 0.2,
                con_alfa(color_de_extension(&ext), alfa),
            );
            if !ext.is_empty() {
                let tam = (caja.alto * 0.3).max(8.0 * e);
                let (w, h) = p.medir_texto(&ext.to_uppercase(), tam);
                p.texto(
                    &ext.to_uppercase(),
                    caja.x + (caja.ancho - w) / 2.0,
                    caja.y + (caja.alto - h) / 2.0,
                    tam,
                    con_alfa(Color::BLANCO, alfa),
                );
            }
        }
    }
}

fn luna(p: &Pintor, c: &Contexto, a: &Astro, v: &Visto, centro: (f32, f32), foco: f32) {
    let e = c.escala;
    let r = v.radio_px * c.escala;
    let Some(f) = c.ficha_de(a) else {
        // Su cuaderno aun no ha llegado: un punto, que ya dice que ahi hay
        // algo sin inventarse lo que es. Muy tenue a proposito: con el gris
        // claro del tema Cosmos a plena opacidad, una galaxia todavia
        // leyendose era una parrilla de bolas palidas que tapaba el cielo.
        p.circulo(
            centro,
            (r * 0.3).max(2.0 * e),
            con_alfa(PALETA.texto_suave, 0.3 * foco),
        );
        return;
    };
    let alfa = foco * opacidad_de_luna(f.en_equipo);
    match v.nivel {
        Nivel::Punto => p.circulo(centro, 2.0 * e, con_alfa(PALETA.texto_suave, alfa)),
        Nivel::Icono => icono_de_luna(p, f, caja_de(centro, r), alfa, e),
        Nivel::Ficha | Nivel::Vista => {
            let caja = caja_de(centro, r);
            p.rellenar_redondeado(caja, 10.0 * e, con_alfa(PALETA.panel, alfa));
            let margen = 8.0 * e;
            let ancho_util = (caja.ancho - 2.0 * margen).max(1.0);
            // La vista previa ocupa la mitad de arriba; en ficha, el icono.
            let alto_arriba = caja.alto * 0.5;
            let arriba = RectF {
                x: caja.x + margen,
                y: caja.y + margen,
                ancho: ancho_util,
                alto: alto_arriba - margen,
            };
            let mut hecho = false;
            if v.nivel == Nivel::Vista {
                if let Some(ruta) = ruta_de_foto(c.raiz, f)
                    && let Some((b, w, h)) = c.miniaturas.ya(&ruta)
                {
                    crate::miniaturas::pintar_recortado(p, b, arriba, w, h);
                    hecho = true;
                } else if matches!(
                    f.clase,
                    ClaseLuna::Nota | ClaseLuna::MiniApp | ClaseLuna::Voz
                ) {
                    p.texto_ajustado(
                        &f.extracto,
                        arriba.x,
                        arriba.y,
                        11.0 * e,
                        arriba.ancho,
                        con_alfa(PALETA.texto_suave, alfa),
                    );
                    hecho = true;
                }
            }
            if !hecho {
                let lado = arriba.alto.min(arriba.ancho);
                icono_de_luna(
                    p,
                    f,
                    RectF {
                        x: arriba.x + (arriba.ancho - lado) / 2.0,
                        y: arriba.y,
                        ancho: lado,
                        alto: lado,
                    },
                    alfa,
                    e,
                );
            }
            let y = caja.y + alto_arriba + 4.0 * e;
            p.texto_linea(
                &f.nombre,
                caja.x + margen,
                y,
                12.0 * e,
                ancho_util,
                con_alfa(PALETA.texto, alfa),
            );
            let mut detalle = Vec::new();
            if f.bytes > 0 {
                detalle.push(tamano_legible(f.bytes));
            }
            if v.nivel == Nivel::Vista
                && let Some(cc) = &f.codigo_chat
            {
                detalle.push(cc.clone());
            }
            if !detalle.is_empty() {
                p.texto_linea(
                    &detalle.join(" · "),
                    caja.x + margen,
                    y + 16.0 * e,
                    11.0 * e,
                    ancho_util,
                    con_alfa(PALETA.texto_suave, alfa),
                );
            }
        }
        _ => {}
    }
    if !f.en_equipo && v.nivel >= Nivel::Icono {
        p.texto(
            "⚠",
            centro.0 + r * 0.55,
            centro.1 - r,
            12.0 * e,
            PALETA.peligro,
        );
    }
}

/// El color con el que se pinta un astro: el de su proyecto si lo tiene, el
/// suyo propio si se lo pusieron, y el acento si no.
fn color_de_astro(c: &Contexto, a: &Astro) -> Color {
    match &a.clase {
        Clase::Galaxia { proyecto } => color_de_sol(proyecto),
        Clase::Luna { proyecto, .. } => color_de_sol(proyecto),
        Clase::Planeta => a.color.map(hex).unwrap_or_else(|| {
            a.padre
                .and_then(|g| c.u.astro(g))
                .and_then(|g| g.proyecto())
                .map(color_de_sol)
                .unwrap_or(PALETA.acento)
        }),
    }
}

/// La punta de flecha en `hasta`, apuntando desde `desde`.
fn punta(desde: (f32, f32), hasta: (f32, f32), largo: f32) -> [(f32, f32); 3] {
    let (dx, dy) = (hasta.0 - desde.0, hasta.1 - desde.1);
    let d = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ux, uy) = (dx / d, dy / d);
    let (bx, by) = (hasta.0 - ux * largo, hasta.1 - uy * largo);
    let (nx, ny) = (-uy * largo * 0.5, ux * largo * 0.5);
    [hasta, (bx + nx, by + ny), (bx - nx, by - ny)]
}

/// Las lineas entre astros (§2.3). Salen del borde de cada circulo.
pub fn conexiones(p: &Pintor, c: &Contexto, camara: &Camara) {
    let e = c.escala;
    for (id, da, db) in pixpin_universo::conexiones_visibles(c.u, c.vistos) {
        let (Some(a), Some(b), Some(con)) = (
            c.u.astro(da),
            c.u.astro(db),
            c.u.conexiones.iter().find(|k| k.id == id),
        ) else {
            continue;
        };
        let (pa, pb) = pixpin_universo::extremos(a, b);
        let (pa, pb) = (
            a_pantalla(camara, pa.0, pa.1),
            a_pantalla(camara, pb.0, pb.1),
        );
        let foco = if c.en_foco(da) || c.en_foco(db) {
            1.0
        } else {
            FUERA_DE_FOCO
        };
        // Como en el movil (`Galaxia.kt:866`): una conexion sin color propio
        // es la media de los colores de los dos astros, no un gris. Asi la
        // linea «pertenece» a los dos y se ve de lejos.
        let base = match con.tipo {
            TipoConexion::Relacion => mezcla(color_de_astro(c, a), color_de_astro(c, b), 0.5),
            _ => PALETA.acento,
        };
        let color = con_alfa(con.color.map(hex).unwrap_or(base), foco);
        // El halo del movil: la misma linea a 3,5 veces de grosor y muy
        // tenue. Es lo que hace que una conexion se lea sobre el cielo sin
        // tener que engordarla.
        let halo = 2.6 * e;
        p.linea(pa, pb, halo * 3.5, con_alfa(color, 0.20));
        match con.tipo {
            TipoConexion::Relacion => p.linea(pa, pb, halo, con_alfa(color, 0.9)),
            TipoConexion::Depende => {
                p.linea(pa, pb, 2.0 * e, color);
                p.poligono(&punta(pa, pb, 10.0 * e), color);
            }
            TipoConexion::Referencia => p.polilinea_discontinua(&[pa, pb], 1.5 * e, color),
            TipoConexion::Secuencia(n) => {
                p.polilinea_discontinua(&[pa, pb], 1.5 * e, color);
                p.poligono(&punta(pa, pb, 10.0 * e), color);
                let medio = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                p.circulo(medio, 9.0 * e, color);
                let t = n.to_string();
                let (w, h) = p.medir_texto(&t, 11.0 * e);
                p.texto(
                    &t,
                    medio.0 - w / 2.0,
                    medio.1 - h / 2.0,
                    11.0 * e,
                    PALETA.espacio,
                );
            }
        }
        if !con.rotulo.is_empty() {
            let medio = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0 + 12.0 * e);
            let (w, _) = p.medir_texto(&con.rotulo, 11.0 * e);
            p.texto_con_fondo(
                &con.rotulo,
                medio.0 - w / 2.0,
                medio.1,
                11.0 * e,
                PALETA.texto,
                PALETA.panel,
            );
        }
    }
}

/// Las nebulosas: lo no colocado de cada galaxia abierta (D228).
///
/// `sueltas_de(proyecto)` da las fichas sin colocar, de la mas nueva a la
/// mas vieja (`nebulosa::sin_colocar`). `rotulo` es «Nebulosa» y `mas(n)`,
/// «+n mas», ya en el idioma del usuario.
pub fn nebulosas<'f>(
    p: &Pintor,
    c: &Contexto,
    camara: &Camara,
    paginas: &HashMap<IdAstro, usize>,
    sueltas_de: impl Fn(&str) -> Vec<&'f FichaLuna>,
    rotulo: &str,
    mas: impl Fn(usize) -> String,
) {
    let e = c.escala;
    for v in c.vistos.iter().filter(|v| v.nivel == Nivel::Vista) {
        let Some(g) = c.u.astro(v.id) else { continue };
        let Clase::Galaxia { proyecto } = &g.clase else {
            continue;
        };
        let sueltas = sueltas_de(proyecto);
        if sueltas.is_empty() {
            continue;
        }
        let pagina = paginas.get(&g.id).copied().unwrap_or(0);
        let tramo = nebulosa::pagina(sueltas.len(), pagina);
        let lado = nebulosa::CELDA * camara.zoom;
        let (x0, y0, _, _) = nebulosa::celda(g, 0);
        let (tx, ty) = a_pantalla(camara, x0, y0);
        p.texto(rotulo, tx, ty - 18.0 * e, 12.0 * e, PALETA.texto_suave);
        for (i, f) in sueltas[tramo.clone()].iter().enumerate() {
            let (cx0, cy0, cx1, cy1) = nebulosa::celda(g, i);
            let (a0, b0) = a_pantalla(camara, cx0, cy0);
            let (a1, b1) = a_pantalla(camara, cx1, cy1);
            let caja = RectF {
                x: a0 + 4.0 * e,
                y: b0 + 4.0 * e,
                ancho: (a1 - a0 - 8.0 * e).max(1.0),
                alto: (b1 - b0 - 8.0 * e).max(1.0),
            };
            let alfa = opacidad_de_luna(f.en_equipo) * 0.85;
            if lado < 48.0 * e {
                icono_de_luna(p, f, caja, alfa, e);
            } else {
                p.rellenar_redondeado(caja, 8.0 * e, con_alfa(PALETA.panel, alfa));
                let lado_icono = caja.alto * 0.5;
                icono_de_luna(
                    p,
                    f,
                    RectF {
                        x: caja.x + (caja.ancho - lado_icono) / 2.0,
                        y: caja.y + 4.0 * e,
                        ancho: lado_icono,
                        alto: lado_icono,
                    },
                    alfa,
                    e,
                );
                p.texto_linea(
                    &f.nombre,
                    caja.x + 4.0 * e,
                    caja.y + caja.alto * 0.62,
                    11.0 * e,
                    caja.ancho - 8.0 * e,
                    con_alfa(PALETA.texto, alfa),
                );
            }
        }
        let resto = sueltas.len().saturating_sub(tramo.end);
        if resto > 0 {
            let (cx0, cy0, cx1, cy1) = nebulosa::chip_mas(g);
            let (a0, b0) = a_pantalla(camara, cx0, cy0);
            let (a1, b1) = a_pantalla(camara, cx1, cy1);
            let caja = RectF {
                x: a0,
                y: b0,
                ancho: (a1 - a0).max(1.0),
                alto: (b1 - b0).max(1.0),
            };
            p.rellenar_redondeado(caja, caja.alto / 2.0, PALETA.panel);
            let t = mas(resto);
            let tam = (12.0 * e).min(caja.alto * 0.5);
            let (w, h) = p.medir_texto(&t, tam);
            p.texto(
                &t,
                caja.x + (caja.ancho - w) / 2.0,
                caja.y + (caja.alto - h) / 2.0,
                tam,
                PALETA.texto,
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::universo::estrellas;

    fn caja(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
        RectF { x, y, ancho, alto }
    }

    #[test]
    fn el_mismo_proyecto_sale_del_mismo_color_que_en_el_movil() {
        // `"".hashCode()` es 0 y `"a".hashCode()` es 97 en Java.
        assert_eq!(hash_java(""), 0);
        assert_eq!(hash_java("a"), 97);
        assert_eq!(hash_java("hola"), 3_208_380);
        // El indice nunca es negativo aunque el hash se desborde.
        for p in ["", "p1", "un id de proyecto bastante largo para desbordar"] {
            assert!(COLORES_DE_SOL.iter().any(|h| hex(*h) == color_de_sol(p)));
        }
        // Caso negativo: dos proyectos distintos no tienen por que coincidir.
        assert_ne!(color_de_sol("p1"), color_de_sol("p2"));
    }

    #[test]
    fn un_rotulo_que_caeria_bajo_la_isla_o_sobre_la_barra_se_va_al_otro_lado() {
        let isla = caja(16.0, 50.0, 120.0, 44.0);
        let techo = 36.0;
        // Sitio libre: se queda donde prefiere.
        assert_eq!(
            y_de_rotulo(600.0, 80.0, 20.0, 200.0, 400.0, isla, techo),
            200.0
        );
        // Debajo de la isla: se va al alternativo.
        assert_eq!(
            y_de_rotulo(60.0, 80.0, 20.0, 60.0, 400.0, isla, techo),
            400.0
        );
        // Por encima del techo: tambien.
        assert_eq!(
            y_de_rotulo(600.0, 80.0, 20.0, 10.0, 400.0, isla, techo),
            400.0
        );
        // Caso negativo: si los dos estorban, se pega al techo y no se
        // pierde arriba.
        assert_eq!(y_de_rotulo(60.0, 80.0, 20.0, 5.0, 60.0, isla, techo), 36.0);
    }

    #[test]
    fn mezclar_da_los_extremos_y_el_medio() {
        let a = Color::NEGRO;
        let b = Color::BLANCO;
        assert_eq!(mezcla(a, b, 0.0), a);
        assert_eq!(mezcla(a, b, 1.0), b);
        assert_eq!(mezcla(a, b, 0.5).r, 0.5);
    }

    #[test]
    fn la_baldosa_de_estrellas_es_la_misma_con_la_misma_semilla() {
        assert_eq!(
            estrellas::puntos(7, 512, 120),
            estrellas::puntos(7, 512, 120)
        );
        assert_ne!(
            estrellas::puntos(7, 512, 120),
            estrellas::puntos(8, 512, 120)
        );
    }

    #[test]
    fn el_paralaje_mueve_la_capa_lejana_menos_que_la_cercana() {
        let lejos = estrellas::desfase(1000.0, 0.2, 512.0);
        let cerca = estrellas::desfase(1000.0, 1.0, 512.0);
        assert_eq!(lejos, (1000.0f32 * 0.2) % 512.0);
        assert_eq!(cerca, 1000.0f32 % 512.0);
    }

    #[test]
    fn una_luna_fantasma_se_pinta_al_cuarenta_por_ciento() {
        assert_eq!(opacidad_de_luna(false), 0.4);
        assert_eq!(opacidad_de_luna(true), 1.0);
    }

    #[test]
    fn la_punta_de_flecha_toca_el_destino_y_mira_hacia_atras() {
        let [a, b, c] = punta((0.0, 0.0), (100.0, 0.0), 10.0);
        assert_eq!(a, (100.0, 0.0));
        assert_eq!((b.0, c.0), (90.0, 90.0));
        assert!(b.1 > 0.0 && c.1 < 0.0 || b.1 < 0.0 && c.1 > 0.0);
    }

    #[test]
    fn solo_una_foto_que_esta_aqui_tiene_ruta_de_miniatura() {
        let raiz = std::path::Path::new("C:/raiz");
        let foto = FichaLuna {
            proyecto: "p".into(),
            clase: ClaseLuna::Imagen,
            ruta: Some("archivos/a.jpg".into()),
            en_equipo: true,
            ..Default::default()
        };
        assert!(ruta_de_foto(raiz, &foto).is_some());
        let fuera = FichaLuna {
            en_equipo: false,
            ..foto.clone()
        };
        assert!(ruta_de_foto(raiz, &fuera).is_none());
        let pdf = FichaLuna {
            clase: ClaseLuna::Archivo,
            ..foto
        };
        assert!(ruta_de_foto(raiz, &pdf).is_none());
    }

    #[test]
    fn el_archivo_se_ensena_con_su_chapa_y_lo_demas_con_su_icono() {
        assert!(icono_de_clase(ClaseLuna::Archivo).is_none());
        assert!(icono_de_clase(ClaseLuna::Imagen).is_some());
        assert_eq!(extension_de("Plano.PDF"), "pdf");
        assert_eq!(extension_de("sin extension"), "");
    }
}
