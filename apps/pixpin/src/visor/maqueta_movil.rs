//! **La maqueta del movil** (K16): donde cae cada bloque de un Word o un
//! libro, con las cuentas de la pagina que ensena el `WebView` de Android.
//!
//! # Por que
//!
//! Lo anotado encima de un documento va en pixeles de su pagina (los del
//! lienzo del movil son pixeles CSS, con el cero en el borde izquierdo de la
//! pagina; los del PC, pixeles logicos con el cero en el borde de la
//! columna). Si los dos aparatos colocan el texto de otra forma, el trazo
//! que en el movil subrayaba una palabra cae en el PC sobre otra: medido con
//! el Word del usuario (`Bibliografia_Papers_Tesis_Max.docx`, columna 384),
//! el movil lo ponia en 12 185 px de alto y el PC en 19 016, asi que el «24»
//! que el usuario escribio encima de las referencias 37 y 38 salia en el PC
//! capitulos mas arriba.
//!
//! Por eso el PC coloca **igual que el movil**: la misma hoja de estilo
//! (`DocxAHtml.ESTILO` para un Word, `EpubAHtml.ESTILO` para un libro, y
//! encima `Lectura.estilo`), la misma letra (Noto Serif y Roboto, las de
//! Android, incrustadas en `pixpin-render`), los mismos renglones de
//! `line-height` exacto (`pixpin_render::lectura`) y las mismas reglas de
//! CSS: margenes que se solapan, relleno de `main`, sangria de las listas,
//! tablas de ancho automatico con sus celdas `padding:4px 8px` y su raya de
//! 1 px, imagenes a su alto. Con la columna fijada (`anot-<uid>.maqueta`)
//! los dos parten los renglones en los mismos sitios y la tinta cae en la
//! misma palabra; el aumento y la ventana solo escalan y desplazan.
//!
//! # Las unidades
//!
//! Las del lector: pixeles logicos a escala 1 con el cero en el borde de la
//! columna (el `body` del movil, de `columna` px de ancho). Dentro va `main`
//! (como mucho 46 em, centrado) con su relleno, y el texto empieza en su
//! caja de contenido. `anotado_del_adjunto` corre la tinta del movil su
//! margen izquierdo (`izq`) al leerla y lo devuelve al guardarla.

use super::tablas::Caja;
use super::*;
use pixpin_docs::documento::{Alineacion as AlineaDoc, Bloque, MARCA_IMAGEN, Trozo};
use pixpin_docs::tabla::Celda;
use pixpin_render::lectura::{Alineacion, LetraDeLectura, Medida};

/// Las letras del movil (`Lectura.LETRAS`: serif, sans-serif, monospace,
/// cursive), con las que las hay aqui: las dos primeras son las de Android
/// incrustadas; Courier New avanza lo mismo que Droid Sans Mono (0,6 em); la
/// cursiva de Android (Dancing Script) no viene, y Caveat es la letra a mano
/// que ya trae la aplicacion.
pub(crate) const FAMILIAS: [&str; lectura::TIPOS] = ["Noto Serif", "Roboto", "Courier New", "Caveat"];

/// Con que letra va un bloque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Letra {
    pub(crate) familia: &'static str,
    pub(crate) peso: u16,
    pub(crate) interlineado: f32,
    pub(crate) alineacion: Alineacion,
}

impl Letra {
    pub(crate) fn para_pintar(&self) -> LetraDeLectura<'static> {
        LetraDeLectura {
            familia: self.familia,
            peso: self.peso,
            interlineado: self.interlineado,
            alineacion: self.alineacion,
        }
    }

    /// La letra del cuerpo de un documento con estos ajustes.
    pub(crate) fn del_cuerpo(hoja: Hoja, a: &lectura::Ajustes) -> Letra {
        Letra {
            familia: FAMILIAS[usize::from(a.tipo).min(FAMILIAS.len() - 1)],
            peso: lectura::PESOS[usize::from(a.grosor).min(lectura::GROSORES - 1)],
            interlineado: hoja.css().interlineado,
            alineacion: Alineacion::Izquierda,
        }
    }
}

/// Lo que mide un parrafo a un ancho con su letra: `visor::medir` lo
/// pregunta al `Pintor`, la hoja de compartir al motor fuera de fotograma y
/// las pruebas a una cuenta de mentira.
pub(crate) type Mide<'a> = dyn Fn(&str, f32, f32, &[Tramo], &Letra) -> Medida + 'a;

/// **Que hoja de estilo usa el movil** para este documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Hoja {
    /// `DocxAHtml.ESTILO` (y lo que no es un libro: pagina, Markdown).
    #[default]
    Word,
    /// `EpubAHtml.ESTILO`.
    Libro,
}

impl Hoja {
    pub(crate) fn de(ruta: &Path) -> Hoja {
        match pixpin_docs::formato_de(&ruta.to_string_lossy()) {
            Some(pixpin_docs::Formato::Epub) => Hoja::Libro,
            _ => Hoja::Word,
        }
    }

    fn css(self) -> &'static Css {
        match self {
            Hoja::Word => &WORD,
            Hoja::Libro => &LIBRO,
        }
    }
}

/// Los numeros de una hoja de estilo del movil. Lo que va en «em» se
/// multiplica por la letra del cuerpo (o la del bloque, en los titulos).
struct Css {
    /// `body{font:16px/1.55 …}`.
    letra: f32,
    interlineado: f32,
    /// `main{max-width:46em;padding:18px 16px 48px}`.
    ancho_max_em: f32,
    arriba: f32,
    lado: f32,
    abajo: f32,
    /// `p{margin:.55em 0}`.
    parrafo_em: f32,
    /// `h1..h6{font-size}` en em del cuerpo.
    titulos: [f32; 6],
    /// `h1..h6{line-height:1.25;margin:1.2em 0 .5em}`, en em del titulo.
    titulo_interlineado: f32,
    titulo_arriba_em: f32,
    titulo_abajo_em: f32,
    /// La lista del Word es `p.lista{margin:.25em 0 .25em 1.2em}`; la del
    /// libro, `ul{margin:1em 0;padding-left:40px}` con `li` sin margen.
    lista_em: f32,
    lista_sangria_em: f32,
    lista_sangria_px: f32,
    /// `.tabla{margin:.8em 0}` del Word; la tabla del libro no lleva.
    tabla_em: f32,
    /// `td p{margin:.2em 0}` del Word; en el libro `p{margin:.7em 0}` tambien
    /// dentro de una celda.
    celda_parrafo_em: f32,
    /// `blockquote{margin:1em 40px}` de fabrica; `1em 1.4em` en el libro.
    cita_em: f32,
    cita_lado_em: f32,
    cita_lado_px: f32,
    /// `hr`: de fabrica `.5em auto` y 2 px de raya; en el libro `1.5em 0` y 1 px.
    regla_em: f32,
    regla_alto: f32,
    /// `section.capitulo{border-top:1px;margin-top:2.5em;padding-top:2em}`.
    capitulo: Option<(f32, f32)>,
    /// `header.libro{margin:2em 0 1em}` con `h1{font-size:1.9em;line-height:1.2;
    /// margin:0 0 .35em}` y el autor en cursiva sin margen.
    cabecera: bool,
}

const WORD: Css = Css {
    letra: 16.0,
    interlineado: 1.55,
    ancho_max_em: 46.0,
    arriba: 18.0,
    lado: 16.0,
    abajo: 48.0,
    parrafo_em: 0.55,
    titulos: [1.7, 1.4, 1.2, 1.05, 1.05, 1.05],
    titulo_interlineado: 1.25,
    titulo_arriba_em: 1.2,
    titulo_abajo_em: 0.5,
    lista_em: 0.25,
    lista_sangria_em: 1.2,
    lista_sangria_px: 0.0,
    tabla_em: 0.8,
    celda_parrafo_em: 0.2,
    cita_em: 1.0,
    cita_lado_em: 0.0,
    cita_lado_px: 40.0,
    regla_em: 0.5,
    regla_alto: 2.0,
    capitulo: None,
    cabecera: false,
};

const LIBRO: Css = Css {
    letra: 17.0,
    interlineado: 1.6,
    ancho_max_em: 42.0,
    arriba: 18.0,
    lado: 18.0,
    abajo: 64.0,
    parrafo_em: 0.7,
    titulos: [1.6, 1.35, 1.15, 1.0, 1.0, 1.0],
    titulo_interlineado: 1.25,
    titulo_arriba_em: 1.3,
    titulo_abajo_em: 0.6,
    lista_em: 0.0,
    lista_sangria_em: 0.0,
    lista_sangria_px: 40.0,
    tabla_em: 0.0,
    celda_parrafo_em: 0.7,
    cita_em: 1.0,
    cita_lado_em: 1.4,
    cita_lado_px: 0.0,
    regla_em: 1.5,
    regla_alto: 1.0,
    capitulo: Some((2.5, 2.0)),
    cabecera: true,
};

/// `td{padding:4px 8px}` y la raya de 1 px de `border-collapse`.
const CELDA_ALTO: f32 = 4.0;
const CELDA_ANCHO: f32 = 8.0;
const RAYA: f32 = 1.0;

/// La letra del cuerpo, en pixeles: la de la hoja por el tamano elegido
/// (el `textZoom` del `WebView`, que escala tambien lo que va en em).
pub(crate) fn letra_del_cuerpo(hoja: Hoja, a: &lectura::Ajustes) -> f32 {
    hoja.css().letra * a.tamano as f32 / 100.0
}

/// **La caja del texto** dentro de la columna: `main` mide la columna o,
/// si es mas ancha, sus 46 em centrados, y el texto va dentro de su relleno.
/// Devuelve `(x, ancho)` en unidades del lector.
pub(crate) fn caja_de_texto(hoja: Hoja, a: &lectura::Ajustes, columna: f32) -> (f32, f32) {
    let css = hoja.css();
    // `max-width` es de la caja de contenido: el relleno va por fuera.
    let texto = (columna - 2.0 * css.lado).min(css.ancho_max_em * letra_del_cuerpo(hoja, a)).max(1.0);
    let x = (columna - texto - 2.0 * css.lado) / 2.0 + css.lado;
    (x, texto)
}

/// Los margenes verticales de CSS: el de abajo de un bloque y el de arriba
/// del siguiente **se solapan** (cuenta el mayor), salvo que haya un relleno
/// o una raya en medio.
struct Flujo {
    y: f32,
    margen: f32,
}

impl Flujo {
    /// Un bloque de `alto` con sus dos margenes: devuelve donde empieza.
    fn bloque(&mut self, arriba: f32, alto: f32, abajo: f32) -> f32 {
        let y = self.y + self.margen.max(arriba);
        self.y = y + alto;
        self.margen = abajo;
        y
    }
}

/// El texto de un bloque y sus tramos, como los pinta el `WebView`: la
/// negrita, la cursiva y el codigo de cada trozo; el enlace solo cambia de
/// color (`.enlace`), no de letra; el tabulador de Word es `&emsp;`, y los
/// espacios seguidos son uno y los del principio y el final no cuentan
/// (`white-space:normal` de HTML).
pub(crate) fn texto_de(trozos: &[Trozo]) -> (String, Vec<Tramo>) {
    let mut limpios: Vec<Trozo> = Vec::with_capacity(trozos.len());
    let mut tras_espacio = true;
    for t in trozos {
        let mut s = String::with_capacity(t.texto.len());
        for ch in t.texto.chars() {
            match ch {
                ' ' | '\r' => {
                    if !tras_espacio {
                        s.push(' ');
                        tras_espacio = true;
                    }
                }
                '\n' => {
                    if s.ends_with(' ') {
                        s.pop();
                    }
                    s.push('\n');
                    tras_espacio = true;
                }
                '\t' => {
                    s.push('\u{2003}');
                    tras_espacio = false;
                }
                c => {
                    s.push(c);
                    tras_espacio = false;
                }
            }
        }
        limpios.push(Trozo { texto: s, estilo: t.estilo });
    }
    if let Some(ultimo) = limpios.iter_mut().rev().find(|t| !t.texto.is_empty())
        && ultimo.texto.ends_with(' ')
    {
        ultimo.texto.pop();
    }
    let b = Bloque::nuevo(Clase::Parrafo, limpios);
    let (texto, lista) = texto_y_tramos(&b);
    let tramos = lista
        .iter()
        .map(|t| Tramo {
            inicio: t.inicio,
            longitud: t.longitud,
            estilo: EstiloTexto {
                negrita: t.estilo.negrita,
                cursiva: t.estilo.cursiva,
                mono: t.estilo.mono,
            },
        })
        .filter(|t| t.estilo != EstiloTexto::default())
        .collect();
    (texto, tramos)
}

fn alineacion(a: AlineaDoc) -> Alineacion {
    match a {
        AlineaDoc::Izquierda => Alineacion::Izquierda,
        AlineaDoc::Centro => Alineacion::Centro,
        AlineaDoc::Derecha => Alineacion::Derecha,
    }
}

/// Todo lo que hace falta para colocar, junto.
struct Obra<'a, 'm> {
    css: &'static Css,
    base: f32,
    cuerpo: Letra,
    x: f32,
    ancho: f32,
    mide: &'a Mide<'m>,
    flujo: Flujo,
    salida: Vec<Colocado>,
}

impl Obra<'_, '_> {
    /// Un parrafo de texto: su alto son sus renglones (uno al menos: el
    /// `<p>&nbsp;</p>` de un parrafo vacio de Word ocupa su renglon).
    #[allow(clippy::too_many_arguments)] // el bloque entero: texto, letra, sitio y margenes
    fn parrafo(
        &mut self,
        texto: String,
        tramos: Vec<Tramo>,
        clase: Clase,
        tam: f32,
        letra: Letra,
        (x, ancho): (f32, f32),
        (arriba, abajo): (f32, f32),
        color: Color,
        bloque: Option<usize>,
    ) {
        let renglon = tam * letra.interlineado;
        let (alto, renglones) = if texto.trim().is_empty() {
            (renglon, Vec::new())
        } else {
            let m = (self.mide)(&texto, tam, ancho, &tramos, &letra);
            (m.alto.max(renglon), m.renglones)
        };
        let y = self.flujo.bloque(arriba, alto, abajo);
        self.salida.push(Colocado {
            texto: if texto.trim().is_empty() { String::new() } else { texto },
            tramos,
            clase,
            tam,
            sangria: x,
            ancho,
            y,
            alto,
            color,
            bloque,
            caja: None,
            letra,
            recuadro: false,
            renglones,
        });
    }
}

/// **Coloca el documento como el movil** en una columna de `columna`
/// unidades. Devuelve los bloques colocados (en orden de lectura) y el alto
/// de la pagina entera.
pub(crate) fn colocar(
    doc: &Documento,
    ajustes: &lectura::Ajustes,
    columna: f32,
    hoja: Hoja,
    mide: &Mide<'_>,
) -> (Vec<Colocado>, f32) {
    let css = hoja.css();
    let base = letra_del_cuerpo(hoja, ajustes);
    let cuerpo = Letra::del_cuerpo(hoja, ajustes);
    let fuerte = cuerpo.peso.max(700);
    let (x, ancho) = caja_de_texto(hoja, ajustes, columna);
    let mut o = Obra {
        css,
        base,
        cuerpo,
        x,
        ancho,
        mide,
        // El relleno de arriba de `main`: el primer margen no lo cruza.
        flujo: Flujo { y: css.arriba, margen: 0.0 },
        salida: Vec::with_capacity(doc.bloques.len() + 2),
    };

    // La cabecera del libro: su titulo centrado y el autor en cursiva.
    if css.cabecera && !doc.titulo.trim().is_empty() {
        let tam = base * 1.9;
        let letra = Letra { peso: fuerte, interlineado: 1.2, alineacion: Alineacion::Centro, ..cuerpo };
        let (texto, tramos) = (doc.titulo.trim().to_string(), Vec::new());
        o.parrafo(texto, tramos, Clase::Titulo(1), tam, letra, (x, ancho), (base * 2.0, tam * 0.35), TEXTO, None);
        if !doc.autor.trim().is_empty() {
            let texto = doc.autor.trim().to_string();
            let largo = texto.encode_utf16().count() as u32;
            let tramos = vec![Tramo {
                inicio: 0,
                longitud: largo,
                estilo: EstiloTexto { cursiva: true, ..EstiloTexto::default() },
            }];
            let letra = Letra { alineacion: Alineacion::Centro, ..cuerpo };
            o.parrafo(texto, tramos, Clase::Nota, base, letra, (x, ancho), (0.0, 0.0), APAGADO, None);
        }
        // El margen de abajo de la cabecera.
        o.flujo.margen = o.flujo.margen.max(base);
    }

    let mut imagen = 0usize;
    let mut i = 0usize;
    while i < doc.bloques.len() {
        let b = &doc.bloques[i];
        match b.clase {
            Clase::Fila => {
                let mut hasta = i + 1;
                while hasta < doc.bloques.len()
                    && pixpin_docs::tabla::misma_tabla(&doc.bloques[hasta - 1], &doc.bloques[hasta])
                {
                    hasta += 1;
                }
                tabla(&mut o, doc, i, hasta);
                i = hasta;
                continue;
            }
            Clase::Capitulo if css.capitulo.is_some() => {
                // La raya de arriba del capitulo y su relleno.
                let (margen, relleno) = css.capitulo.unwrap_or((2.5, 2.0));
                let y = o.flujo.bloque(base * margen, RAYA + base * relleno, 0.0);
                o.salida.push(raya(Clase::Capitulo, x, ancho, y, Some(i), cuerpo));
            }
            Clase::Regla | Clase::Capitulo => {
                let y = o.flujo.bloque(base * css.regla_em, css.regla_alto, base * css.regla_em);
                o.salida.push(raya(b.clase, x, ancho, y, Some(i), cuerpo));
            }
            Clase::Nota if b.texto() == MARCA_IMAGEN => {
                let datos = doc.imagenes.get(imagen);
                imagen += 1;
                colocar_imagen(&mut o, datos, i);
            }
            Clase::Titulo(n) => {
                let tam = base * css.titulos[usize::from(n.clamp(1, 6)) - 1];
                let letra = Letra {
                    peso: fuerte,
                    interlineado: css.titulo_interlineado,
                    alineacion: alineacion(b.alineacion),
                    ..cuerpo
                };
                let (texto, tramos) = texto_de(&b.trozos);
                let margenes = (tam * css.titulo_arriba_em, tam * css.titulo_abajo_em);
                o.parrafo(texto, tramos, b.clase, tam, letra, (x, ancho), margenes, TEXTO, Some(i));
            }
            Clase::Lista => {
                let (texto, tramos) = texto_de(&b.trozos);
                // La vineta va delante, en el renglon: corre los tramos.
                let texto = format!("• {texto}");
                let tramos = tramos.into_iter().map(|t| Tramo { inicio: t.inicio + 2, ..t }).collect();
                let sangria = base * css.lista_sangria_em + css.lista_sangria_px;
                // En el libro los puntos seguidos son una lista: su margen
                // va antes del primero y despues del ultimo.
                let (arriba, abajo) = if css.lista_em > 0.0 {
                    (base * css.lista_em, base * css.lista_em)
                } else {
                    let antes = i > 0 && doc.bloques[i - 1].clase == Clase::Lista;
                    let despues = doc.bloques.get(i + 1).is_some_and(|s| s.clase == Clase::Lista);
                    (if antes { 0.0 } else { base }, if despues { 0.0 } else { base })
                };
                let letra = Letra { alineacion: alineacion(b.alineacion), ..cuerpo };
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    base,
                    letra,
                    (x + sangria, (ancho - sangria).max(1.0)),
                    (arriba, abajo),
                    TEXTO,
                    Some(i),
                );
            }
            Clase::Cita => {
                let lado = base * css.cita_lado_em + css.cita_lado_px;
                let (texto, tramos) = texto_de(&b.trozos);
                o.parrafo(
                    texto,
                    tramos,
                    b.clase,
                    base,
                    cuerpo,
                    (x + lado, (ancho - 2.0 * lado).max(1.0)),
                    (base * css.cita_em, base * css.cita_em),
                    APAGADO,
                    Some(i),
                );
            }
            Clase::Codigo => {
                // `pre` no esta en la lista de `Lectura.estilo`: se queda en
                // la de ancho fijo del navegador, a 13 px (por el tamano).
                let tam = 13.0 * ajustes.tamano as f32 / 100.0;
                let letra = Letra { familia: pixpin_render::lectura::LETRA_FIJA, peso: 400, ..cuerpo };
                let (texto, tramos) = texto_de(&b.trozos);
                o.parrafo(texto, tramos, b.clase, tam, letra, (x, ancho), (tam, tam), TEXTO, Some(i));
            }
            Clase::Parrafo | Clase::Nota => {
                let (texto, tramos) = texto_de(&b.trozos);
                let color = if b.clase == Clase::Nota { APAGADO } else { TEXTO };
                let letra = Letra { alineacion: alineacion(b.alineacion), ..cuerpo };
                let m = base * css.parrafo_em;
                o.parrafo(texto, tramos, b.clase, base, letra, (x, ancho), (m, m), color, Some(i));
            }
        }
        i += 1;
    }
    // El ultimo margen no cruza el relleno de abajo de `main`.
    let alto = o.flujo.y + o.flujo.margen + o.css.abajo;
    (o.salida, alto)
}

fn raya(clase: Clase, x: f32, ancho: f32, y: f32, bloque: Option<usize>, letra: Letra) -> Colocado {
    Colocado {
        texto: String::new(),
        tramos: Vec::new(),
        clase,
        tam: 0.0,
        sangria: x,
        ancho,
        y,
        alto: RAYA,
        color: super::RAYA,
        bloque,
        caja: None,
        letra,
        recuadro: false,
        renglones: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Imagenes

/// **Lo que mide una imagen** por su cabecera, sin descodificarla: PNG, GIF,
/// BMP, JPEG y WebP, que son las que pinta el `WebView`.
pub(crate) fn medidas_de_imagen(d: &[u8]) -> Option<(u32, u32)> {
    let u32be = |i: usize| d.get(i..i + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let u16be = |i: usize| d.get(i..i + 2).map(|b| u32::from(u16::from_be_bytes([b[0], b[1]])));
    let u16le = |i: usize| d.get(i..i + 2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])));
    let i32le = |i: usize| d.get(i..i + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]).unsigned_abs());
    let medidas = if d.starts_with(b"\x89PNG") {
        (u32be(16)?, u32be(20)?)
    } else if d.starts_with(b"GIF8") {
        (u16le(6)?, u16le(8)?)
    } else if d.starts_with(b"BM") {
        (i32le(18)?, i32le(22)?)
    } else if d.starts_with(b"RIFF") && d.get(8..12) == Some(b"WEBP") {
        match d.get(12..16)? {
            b"VP8X" => {
                let tres = |i: usize| d.get(i..i + 3).map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]) + 1);
                (tres(24)?, tres(27)?)
            }
            b"VP8L" => {
                let b = d.get(21..25)?;
                let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                ((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1)
            }
            _ => (u16le(26)? & 0x3FFF, u16le(28)? & 0x3FFF),
        }
    } else if d.starts_with(&[0xFF, 0xD8]) {
        // Los segmentos del JPEG hasta el que dice su tamano (SOFn).
        let mut i = 2usize;
        loop {
            if *d.get(i)? != 0xFF {
                return None;
            }
            let marca = *d.get(i + 1)?;
            if marca == 0xFF {
                i += 1;
                continue;
            }
            let largo = u16be(i + 2)? as usize;
            if matches!(marca, 0xC0..=0xCF) && !matches!(marca, 0xC4 | 0xC8 | 0xCC) {
                break (u16be(i + 7)?, u16be(i + 5)?);
            }
            i += 2 + largo;
        }
    } else {
        return None;
    };
    (medidas.0 > 0 && medidas.1 > 0).then_some(medidas)
}

/// Una imagen del documento: en el movil va dentro de su `<p>`, a su tamano
/// y como mucho del ancho del texto (`img{max-width:100%;height:auto}`),
/// sentada en la linea base del renglon, asi que debajo queda lo que baja la
/// letra. Aqui se deja su hueco con un recuadro que dice que imagen es: se ve
/// entera al guardar la pagina.
fn colocar_imagen(o: &mut Obra, datos: Option<&pixpin_docs::documento::Imagen>, i: usize) {
    let base = o.base;
    let renglon = base * o.cuerpo.interlineado;
    let (w, h) = datos
        .and_then(|d| medidas_de_imagen(&d.datos))
        .map(|(w, h)| {
            let w = w as f32;
            let ancho = w.min(o.ancho);
            (ancho, h as f32 * ancho / w)
        })
        .unwrap_or((o.ancho, renglon));
    // Lo que queda del renglon por debajo de la linea base: medio
    // interlineado y la bajada de la letra (una quinta parte del cuerpo).
    let debajo = (renglon - base) / 2.0 + base * 0.2;
    let alto = (h + debajo).max(renglon);
    let m = base * o.css.parrafo_em;
    let y = o.flujo.bloque(m, alto, m);
    let peso = datos.map(|d| d.datos.len() / 1024).unwrap_or(0);
    o.salida.push(Colocado {
        texto: format!("🖼 imagen ({peso} kB) — se ve al guardar la página"),
        tramos: Vec::new(),
        clase: Clase::Nota,
        tam: base * 0.85,
        sangria: o.x,
        ancho: w,
        y,
        alto: h.max(1.0),
        color: APAGADO,
        bloque: Some(i),
        caja: None,
        letra: o.cuerpo,
        recuadro: true,
        renglones: Vec::new(),
    });
}

// ---------------------------------------------------------------------------
// Tablas

/// Los parrafos de una celda: el movil pone cada parrafo de Word en su `<p>`
/// (y uno vacio, `<p>&nbsp;</p>`, ocupa su renglon); la continuacion de una
/// celda unida hacia abajo es un `<td></td>` sin nada.
fn parrafos_de_celda(c: &Celda) -> Vec<(String, Vec<Tramo>)> {
    if c.sigue {
        return Vec::new();
    }
    let mut parrafos: Vec<Vec<Trozo>> = vec![Vec::new()];
    for t in &c.trozos {
        let mut partes = t.texto.split('\n');
        if let Some(primera) = partes.next()
            && !primera.is_empty()
        {
            parrafos.last_mut().expect("siempre hay uno").push(Trozo { texto: primera.to_string(), estilo: t.estilo });
        }
        for parte in partes {
            parrafos.push(Vec::new());
            if !parte.is_empty() {
                parrafos.last_mut().expect("siempre hay uno").push(Trozo { texto: parte.to_string(), estilo: t.estilo });
            }
        }
    }
    parrafos.iter().map(|p| texto_de(p)).collect()
}

/// **El reparto de columnas de una tabla automatica**, como el del
/// navegador (LayoutNG): cada columna tiene su minimo (su palabra mas
/// ancha) y su maximo (su parrafo en un renglon). La tabla mide su maximo
/// si cabe; si no, lo que haya, pero nunca menos que su minimo (entonces se
/// sale por la derecha, como en el movil). Lo que haya entre el minimo y el
/// ancho se reparte en proporcion a lo que a cada columna le falta para su
/// maximo.
pub(crate) fn repartir_columnas(minimo: &[f32], maximo: &[f32], disponible: f32) -> Vec<f32> {
    let suma_min: f32 = minimo.iter().sum();
    let suma_max: f32 = maximo.iter().zip(minimo).map(|(a, b)| a.max(*b)).sum();
    if suma_max <= disponible {
        return maximo.iter().zip(minimo).map(|(a, b)| a.max(*b)).collect();
    }
    if suma_min >= disponible || suma_max <= suma_min {
        return minimo.to_vec();
    }
    let sobra = disponible - suma_min;
    let falta = suma_max - suma_min;
    minimo
        .iter()
        .zip(maximo)
        .map(|(mi, ma)| mi + (ma.max(*mi) - mi) * sobra / falta)
        .collect()
}

/// Coloca las filas `desde..hasta` como una tabla del movil.
fn tabla(o: &mut Obra, doc: &Documento, desde: usize, hasta: usize) {
    let base = o.base;
    let cuerpo = o.cuerpo;
    let filas: Vec<Vec<Celda>> = doc.bloques[desde..hasta].iter().map(pixpin_docs::tabla::celdas_de).collect();
    let n = pixpin_docs::tabla::columnas_de(&filas).max(1);
    let parrafos: Vec<Vec<Vec<(String, Vec<Tramo>)>>> =
        filas.iter().map(|f| f.iter().map(parrafos_de_celda).collect()).collect();

    // Minimo y maximo de cada columna, por sus celdas de una columna.
    let relleno = 2.0 * CELDA_ANCHO + RAYA;
    let mut minimo = vec![relleno; n];
    let mut maximo = vec![relleno; n];
    let mut anchas: Vec<(usize, usize, f32, f32)> = Vec::new();
    for (f, ps) in filas.iter().zip(&parrafos) {
        let mut g = 0usize;
        for (c, pc) in f.iter().zip(ps) {
            let span = usize::from(c.columnas.max(1));
            let (mut mi, mut ma) = (0.0f32, 0.0f32);
            for (t, tramos) in pc {
                if t.trim().is_empty() {
                    continue;
                }
                let m = (o.mide)(t, base, 1.0e6, tramos, &cuerpo);
                mi = mi.max(m.minimo);
                ma = ma.max(m.ancho);
            }
            if g < n {
                if span == 1 {
                    minimo[g] = minimo[g].max(mi + relleno);
                    maximo[g] = maximo[g].max(ma + relleno);
                } else {
                    anchas.push((g, (g + span).min(n), mi + relleno, ma + relleno));
                }
            }
            g += span;
        }
    }
    // Una celda de varias columnas que no cabe en ellas las ensancha a partes iguales.
    for (a, b, mi, ma) in anchas {
        let k = (b - a).max(1) as f32;
        let (sm, sx): (f32, f32) = (minimo[a..b].iter().sum(), maximo[a..b].iter().sum());
        if mi > sm {
            minimo[a..b].iter_mut().for_each(|v| *v += (mi - sm) / k);
        }
        if ma > sx {
            maximo[a..b].iter_mut().for_each(|v| *v += (ma - sx) / k);
        }
    }
    for (mi, ma) in minimo.iter().zip(maximo.iter_mut()) {
        *ma = ma.max(*mi);
    }
    let anchos = repartir_columnas(&minimo, &maximo, o.ancho - RAYA);
    let mut xs = Vec::with_capacity(n + 1);
    xs.push(o.x);
    for a in &anchos {
        xs.push(xs.last().copied().unwrap_or(o.x) + a);
    }

    let arriba = base * o.css.tabla_em;
    let margen_p = base * o.css.celda_parrafo_em;
    let renglon = base * cuerpo.interlineado;
    // La tabla es un bloque: su margen se solapa con el de lo de antes.
    let y0 = o.flujo.bloque(arriba, 0.0, 0.0);
    // Con `border-collapse` la raya de arriba es media dentro de la tabla:
    // la primera fila empieza medio pixel mas abajo. La de abajo es la de la
    // ultima fila, que ya va en su alto.
    let mut y = y0 + RAYA / 2.0;
    for (k, (f, ps)) in filas.iter().zip(&parrafos).enumerate() {
        let i = desde + k;
        let abajo_sigue: Vec<usize> = filas
            .get(k + 1)
            .map(|s| {
                let mut g = 0usize;
                s.iter()
                    .filter_map(|c| {
                        let r = (g, c.sigue);
                        g += usize::from(c.columnas.max(1));
                        r.1.then_some(r.0)
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Primero se mide cada celda a su ancho: la fila mide la mas alta.
        let mut celdas: Vec<(usize, usize, Vec<(String, Vec<Tramo>, f32, Vec<(u32, u32)>)>, f32)> = Vec::new();
        let mut g = 0usize;
        for (c, pc) in f.iter().zip(ps) {
            let span = usize::from(c.columnas.max(1));
            if g >= n {
                break;
            }
            let fin = (g + span).min(n);
            let ancho = (xs[fin] - xs[g] - relleno).max(1.0);
            let mut dentro = Vec::with_capacity(pc.len());
            let mut alto = 0.0f32;
            for (t, tramos) in pc {
                let (h, renglones) = if t.trim().is_empty() {
                    (renglon, Vec::new())
                } else {
                    let m = (o.mide)(t, base, ancho, tramos, &cuerpo);
                    (m.alto.max(renglon), m.renglones)
                };
                alto += h;
                dentro.push((t.clone(), tramos.clone(), h, renglones));
            }
            if !dentro.is_empty() {
                // Los margenes de sus parrafos: arriba, entre ellos y abajo
                // (la celda es su propio bloque: no se escapan de ella).
                alto += margen_p * (dentro.len() + 1) as f32;
            }
            celdas.push((g, fin, dentro, alto));
            let _ = c;
            g = fin;
        }
        let contenido = celdas.iter().map(|c| c.3).fold(0.0f32, f32::max);
        let alto_fila = contenido + 2.0 * CELDA_ALTO + RAYA;
        for ((g, fin, dentro, _), c) in celdas.into_iter().zip(f.iter()) {
            let (x, ancho) = (xs[g], xs[fin] - xs[g]);
            let caja = Caja {
                x,
                ancho: ancho + RAYA,
                y,
                alto: alto_fila + RAYA,
                arriba: !c.sigue,
                abajo: !abajo_sigue.contains(&g),
                relleno: c.relleno,
            };
            let mut ty = y + RAYA + CELDA_ALTO + margen_p;
            let texto_x = x + RAYA + CELDA_ANCHO;
            let texto_ancho = (ancho - relleno).max(1.0);
            if dentro.is_empty() {
                // Una celda sin nada: solo su caja.
                o.salida.push(celda_vacia(caja, i, cuerpo, texto_x, texto_ancho, ty, base));
                continue;
            }
            for (n_p, (t, tramos, h, renglones)) in dentro.into_iter().enumerate() {
                o.salida.push(Colocado {
                    texto: if t.trim().is_empty() { String::new() } else { t },
                    tramos,
                    clase: Clase::Fila,
                    tam: base,
                    sangria: texto_x,
                    ancho: texto_ancho,
                    y: ty,
                    alto: h,
                    color: TEXTO,
                    bloque: Some(i),
                    caja: (n_p == 0).then_some(caja),
                    letra: cuerpo,
                    recuadro: false,
                    renglones,
                });
                ty += h + margen_p;
            }
        }
        y += alto_fila;
    }
    let alto = y - y0;
    o.flujo.y = y0 + alto;
    o.flujo.margen = base * o.css.tabla_em;
}

fn celda_vacia(caja: Caja, i: usize, letra: Letra, x: f32, ancho: f32, y: f32, base: f32) -> Colocado {
    Colocado {
        texto: String::new(),
        tramos: Vec::new(),
        clase: Clase::Fila,
        tam: base,
        sangria: x,
        ancho,
        y,
        alto: 0.0,
        color: TEXTO,
        bloque: Some(i),
        caja: Some(caja),
        letra,
        recuadro: false,
        renglones: Vec::new(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::tabla::FilaDeTabla;

    /// Una medida de mentira: 8 unidades por letra, renglones de la letra
    /// por su interlineado; el minimo es la palabra mas larga.
    fn mide(texto: &str, tam: f32, ancho: f32, _t: &[Tramo], l: &Letra) -> Medida {
        let letras = texto.chars().count() as f32 * 8.0;
        let palabra = texto.split_whitespace().map(|p| p.chars().count()).max().unwrap_or(0) as f32 * 8.0;
        let renglones = texto
            .split('\n')
            .map(|p| ((p.chars().count() as f32 * 8.0) / ancho.max(1.0)).ceil().max(1.0))
            .sum::<f32>();
        Medida { ancho: letras.min(ancho), alto: renglones * tam * l.interlineado, minimo: palabra, renglones: Vec::new() }
    }

    fn p(t: &str) -> Bloque {
        Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(t)])
    }

    fn a384() -> lectura::Ajustes {
        lectura::Ajustes { columna: 384, tipo: 0, grosor: 1, ..lectura::Ajustes::default() }
    }

    #[test]
    fn un_word_empieza_en_el_relleno_de_main_y_los_margenes_de_los_parrafos_se_solapan() {
        let doc = Documento {
            titulo: "no se ve en un Word".into(),
            bloques: vec![p("uno"), p("dos")],
            ..Documento::default()
        };
        let (c, alto) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
        assert_eq!(c.len(), 2, "el movil no pone el titulo del fichero en la pagina de un Word");
        // `main{padding-top:18px}` y el `p{margin:.55em}` del primero.
        assert!((c[0].y - (18.0 + 8.8)).abs() < 1e-3, "{}", c[0].y);
        // Entre dos parrafos, un solo margen (el mayor), no la suma.
        assert!((c[1].y - (c[0].y + 24.8 + 8.8)).abs() < 1e-3, "{}", c[1].y);
        // El texto empieza en el relleno de `main` (16 px) y mide la columna menos los dos.
        assert_eq!((c[0].sangria, c[0].ancho), (16.0, 352.0));
        // Abajo: el ultimo margen y el relleno de 48.
        assert!((alto - (c[1].y + 24.8 + 8.8 + 48.0)).abs() < 1e-3, "{alto}");
    }

    #[test]
    fn en_una_columna_ancha_main_se_queda_en_sus_46_em_y_va_centrado() {
        let a = lectura::Ajustes { columna: 860, ..a384() };
        let (x, ancho) = caja_de_texto(Hoja::Word, &a, 860.0);
        // Medido en el Chromium: el texto mide 736 y empieza en 62.
        assert_eq!(ancho, 46.0 * 16.0);
        assert_eq!(x, (860.0 - 736.0 - 32.0) / 2.0 + 16.0);
        // Con la letra mas grande, main (en em) tambien crece.
        let grande = lectura::Ajustes { tamano: 130, ..a };
        assert_eq!(caja_de_texto(Hoja::Word, &grande, 860.0), (16.0, 860.0 - 32.0));
        // Caso negativo: en una columna estrecha manda la columna.
        assert_eq!(caja_de_texto(Hoja::Word, &a384(), 384.0), (16.0, 352.0));
    }

    #[test]
    fn un_titulo_lleva_su_margen_en_em_de_su_letra_y_su_interlineado_de_titulo() {
        let doc = Documento {
            bloques: vec![p("antes"), Bloque::nuevo(Clase::Titulo(1), vec![Trozo::llano("Titulo")]), p("despues")],
            ..Documento::default()
        };
        let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
        let tam = 16.0 * 1.7;
        assert_eq!(c[1].tam, tam);
        assert_eq!(c[1].letra.peso, 700);
        // El margen de arriba del h1 (1.2 em de 27.2) gana al del parrafo.
        assert!((c[1].y - (c[0].y + 24.8 + tam * 1.2)).abs() < 1e-3);
        assert!((c[1].alto - tam * 1.25).abs() < 1e-3);
        // Debajo, el .5em del h1 (13.6) gana al .55em del parrafo (8.8).
        assert!((c[2].y - (c[1].y + tam * 1.25 + tam * 0.5)).abs() < 1e-3);
    }

    #[test]
    fn un_parrafo_vacio_de_word_ocupa_su_renglon_como_el_nbsp_del_movil() {
        let doc = Documento { bloques: vec![p("a"), p(""), p("b")], ..Documento::default() };
        let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
        assert_eq!(c.len(), 3);
        assert!(c[1].texto.is_empty());
        assert!((c[2].y - c[0].y - 2.0 * (24.8 + 8.8)).abs() < 1e-3);
    }

    #[test]
    fn la_letra_y_el_grosor_son_los_cuatro_del_movil() {
        for (tipo, familia) in [(0u8, "Noto Serif"), (1, "Roboto"), (2, "Courier New"), (3, "Caveat")] {
            let a = lectura::Ajustes { tipo, ..a384() };
            assert_eq!(Letra::del_cuerpo(Hoja::Word, &a).familia, familia);
        }
        let a = lectura::Ajustes { grosor: 3, ..a384() };
        assert_eq!(Letra::del_cuerpo(Hoja::Word, &a).peso, 800);
        assert_eq!(Letra::del_cuerpo(Hoja::Libro, &a).interlineado, 1.6);
    }

    #[test]
    fn el_enlace_no_cambia_la_letra_y_el_tabulador_es_un_espacio_eme() {
        let enlace = Trozo {
            texto: "https://x.pe".into(),
            estilo: pixpin_docs::documento::Estilo { enlace: true, ..Default::default() },
        };
        let (t, tramos) = texto_de(&[Trozo::llano("a\tb "), enlace]);
        assert_eq!(t, "a\u{2003}b https://x.pe");
        assert!(tramos.is_empty(), "el movil solo le cambia el color: {tramos:?}");
    }

    #[test]
    fn el_reparto_de_la_tabla_da_el_maximo_si_cabe_y_el_minimo_si_no_cabe_ni_eso() {
        assert_eq!(repartir_columnas(&[20.0, 30.0], &[50.0, 60.0], 300.0), vec![50.0, 60.0]);
        assert_eq!(repartir_columnas(&[200.0, 300.0], &[500.0, 600.0], 300.0), vec![200.0, 300.0]);
        // Entre medias, lo que sobra en proporcion a lo que falta.
        let r = repartir_columnas(&[20.0, 20.0], &[120.0, 40.0], 100.0);
        assert!((r[0] + r[1] - 100.0).abs() < 1e-3);
        assert!((r[0] - (20.0 + 100.0 * 60.0 / 120.0)).abs() < 1e-3, "{r:?}");
    }

    fn fila(celdas: Vec<Celda>) -> Bloque {
        let mut b = Bloque::nuevo(Clase::Fila, vec![Trozo::llano("x")]);
        b.fila = Some(FilaDeTabla { tabla: 0, rejilla: vec![100, 9000], pagina: 10_000, celdas });
        b
    }

    #[test]
    fn una_tabla_del_word_ignora_la_rejilla_y_cada_parrafo_de_celda_lleva_su_margen() {
        let doc = Documento {
            bloques: vec![
                p("antes"),
                fila(vec![Celda::llana(vec![Trozo::llano("N")]), Celda::llana(vec![Trozo::llano("uno\n\ndos")])]),
                p("despues"),
            ],
            ..Documento::default()
        };
        let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
        let celdas: Vec<&Colocado> = c.iter().filter(|x| x.clase == Clase::Fila).collect();
        // Tres parrafos en la segunda celda (el vacio en medio cuenta).
        assert_eq!(celdas.len(), 4);
        let k = celdas[0].caja.expect("la primera lleva la caja");
        // La columna mide lo que su texto (1 letra) mas su relleno, no la rejilla de Word.
        assert!((k.ancho - (8.0 + 17.0 + 1.0)).abs() < 1e-3, "{k:?}");
        // Alto de la fila: 3 renglones, 4 margenes de .2em, relleno y raya.
        let contenido = 3.0 * 24.8 + 4.0 * 3.2;
        assert!((k.alto - (contenido + 8.0 + 2.0)).abs() < 1e-3, "{k:?}");
        // El parrafo de despues, tras la tabla y su margen de .8em.
        let despues = c.last().unwrap();
        // (la fila con su raya de abajo; la de arriba es media fuera, como en
        // el Chromium).
        assert!((despues.y - (k.y + contenido + 8.0 + 1.0 + 12.8)).abs() < 1e-3, "{} {k:?}", despues.y);
    }

    #[test]
    fn un_libro_lleva_su_cabecera_y_la_raya_de_cada_capitulo() {
        let doc = Documento {
            titulo: "El extranjero".into(),
            autor: "Camus".into(),
            bloques: vec![Bloque::nuevo(Clase::Capitulo, vec![]), p("Hoy ha muerto mama.")],
            imagenes: Vec::new(),
        };
        let a = lectura::Ajustes { tipo: 0, ..lectura::Ajustes::default() };
        let (c, _) = colocar(&doc, &a, 600.0, Hoja::Libro, &mide);
        assert_eq!(c[0].bloque, None, "la cabecera no es un bloque del documento");
        assert_eq!(c[0].letra.alineacion, Alineacion::Centro);
        // 18 de relleno y 2 em de margen de la cabecera.
        assert!((c[0].y - (18.0 + 34.0)).abs() < 1e-3);
        let raya = c.iter().find(|x| x.clase == Clase::Capitulo).expect("raya");
        let texto = c.last().unwrap();
        // Tras la raya, su relleno de 2 em sin solaparse con el .7em del parrafo.
        assert!((texto.y - (raya.y + 1.0 + 34.0 + 17.0 * 0.7)).abs() < 1e-3, "{} {}", texto.y, raya.y);
    }

    #[test]
    fn las_medidas_de_una_imagen_salen_de_su_cabecera() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(medidas_de_imagen(&png), Some((640, 480)));
        let gif = [b"GIF89a".as_slice(), &[0x20, 0x01, 0x10, 0x00]].concat();
        assert_eq!(medidas_de_imagen(&gif), Some((288, 16)));
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0x01, 0xE0, 0x02, 0x80];
        assert_eq!(medidas_de_imagen(&jpg), Some((640, 480)));
        // Casos negativos: lo que no es una imagen, o esta cortado.
        assert_eq!(medidas_de_imagen(b"hola"), None);
        assert_eq!(medidas_de_imagen(&png[..18]), None);
    }

    #[test]
    fn una_imagen_ancha_se_encoge_al_texto_y_ocupa_su_alto() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&704u32.to_be_bytes());
        png.extend_from_slice(&352u32.to_be_bytes());
        let doc = Documento {
            bloques: vec![Bloque::nota(MARCA_IMAGEN), p("pie")],
            imagenes: vec![pixpin_docs::documento::Imagen { mime: "image/png".into(), datos: png }],
            ..Documento::default()
        };
        let (c, _) = colocar(&doc, &a384(), 384.0, Hoja::Word, &mide);
        assert!(c[0].recuadro);
        assert_eq!((c[0].ancho, c[0].alto), (352.0, 176.0));
        assert!(c[1].y > c[0].y + 176.0 + 8.8);
    }
}

/// **Contra la pagina del movil de verdad** (K16): el Word del usuario (una
/// copia) puesto en la pagina de `DocxAHtml` con la hoja de `Lectura.estilo`
/// y las letras de Android, medido en un Chromium sin cabeza (el mismo motor
/// que el `WebView`). `PIXPIN_TOPS` es el JSON `{t:[..],h:..}` que deja su
/// guion `MEDIR` y `PIXPIN_DOCX` el Word. Se lanza a mano:
/// `cargo test -p pixpin --bin pixpinmax contra_la_pagina_del_movil -- --ignored --nocapture`.
#[cfg(test)]
mod contra_el_movil {
    use super::*;

    /// Lo que mide el guion del movil (`p,h1..h6,li,tr,img,…` en orden del
    /// documento), sacado de lo colocado: cada parrafo su texto, cada fila su
    /// caja y dentro sus parrafos. `(y, alto, ancho, x)` en pixeles de la
    /// pagina del movil (la x desde el borde de la columna mas `izq`).
    pub(crate) fn como_el_movil(c: &[Colocado], izq: f32) -> Vec<(f32, f32, f32, f32, String)> {
        let mut v = Vec::new();
        let mut fila: Option<usize> = None;
        let mut celdas: Vec<&Colocado> = Vec::new();
        let vaciar = |celdas: &mut Vec<&Colocado>, v: &mut Vec<(f32, f32, f32, f32, String)>| {
            let cajas: Vec<Caja> = celdas.iter().filter_map(|x| x.caja).collect();
            if let (Some(a), Some(b)) = (cajas.first(), cajas.last()) {
                v.push((a.y, a.alto, b.x + b.ancho - a.x, a.x + izq, "TR".into()));
            }
            for x in celdas.iter().filter(|x| !(x.caja.is_some() && x.alto == 0.0)) {
                v.push((x.y, x.alto, x.ancho, x.sangria + izq, x.texto.chars().take(40).collect()));
            }
            celdas.clear();
        };
        for x in c {
            if x.clase == Clase::Fila {
                if fila != x.bloque {
                    vaciar(&mut celdas, &mut v);
                    fila = x.bloque;
                }
                celdas.push(x);
                continue;
            }
            vaciar(&mut celdas, &mut v);
            fila = None;
            if x.bloque.is_some() && x.clase != Clase::Capitulo {
                v.push((x.y, x.alto, x.ancho, x.sangria + izq, x.texto.chars().take(40).collect()));
            }
        }
        vaciar(&mut celdas, &mut v);
        v
    }

    #[test]
    #[ignore = "necesita la copia del Word y las medidas del Chromium"]
    fn contra_la_pagina_del_movil() {
        let docx = std::env::var("PIXPIN_DOCX").expect("PIXPIN_DOCX");
        let tops = std::env::var("PIXPIN_TOPS").expect("PIXPIN_TOPS");
        let columna: f32 = std::env::var("PIXPIN_COLUMNA").ok().and_then(|c| c.parse().ok()).unwrap_or(384.0);
        let izq: f32 = std::env::var("PIXPIN_IZQ").ok().and_then(|c| c.parse().ok()).unwrap_or(256.0);
        let tipo: u8 = std::env::var("PIXPIN_TIPO").ok().and_then(|c| c.parse().ok()).unwrap_or(0);
        let todo = std::env::var("PIXPIN_TODO").is_ok();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(tops).unwrap()).unwrap();
        let movil: Vec<f32> = v["t"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect();
        let datos = v["d"].as_array().cloned().unwrap_or_default();
        let alto_movil = v["h"].as_f64().unwrap() as f32;
        let doc = pixpin_docs::abrir(Path::new(&docx)).unwrap();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
        let mide = |t: &str, tam: f32, ancho: f32, tramos: &[Tramo], l: &Letra| {
            motor.medir_de_lectura(t, tam, ancho, tramos, &l.para_pintar())
        };
        let a = lectura::Ajustes { columna: columna as u32, tipo, grosor: 1, ..lectura::Ajustes::default() };
        let (c, alto) = colocar(&doc, &a, columna, Hoja::Word, &mide);
        let pc = como_el_movil(&c, izq);
        println!("movil: {} tops, alto {alto_movil}; PC: {} tops, alto {alto}", movil.len(), pc.len());
        let n = movil.len().min(pc.len());
        let mut peor = (0usize, 0.0f32);
        let mut antes = 0.0f32;
        for i in 0..n {
            let dif = pc[i].0 - movil[i];
            if dif.abs() > peor.1 {
                peor = (i, dif.abs());
            }
            let m = datos.get(i).cloned().unwrap_or_default();
            let (ma, mw, mx) = (m[1].as_f64().unwrap_or(0.0) as f32, m[2].as_f64().unwrap_or(0.0) as f32, m[3].as_f64().unwrap_or(0.0) as f32);
            let salto = (dif - antes).abs() > 0.6 || (pc[i].1 - ma).abs() > 0.6 || (pc[i].2 - mw).abs() > 1.5;
            if todo || i < 6 || salto {
                println!(
                    "{i:4} {:>3}: y {:8.1}/{:8.1} ({:+6.1}) alto {:6.1}/{:6.1} ancho {:6.1}/{:6.1} x {:6.1}/{:6.1} | {}",
                    m[0].as_str().unwrap_or(""), movil[i], pc[i].0, dif, ma, pc[i].1, mw, pc[i].2, mx, pc[i].3, pc[i].4
                );
            }
            antes = dif;
        }
        let medias: f32 = (0..n).map(|i| (pc[i].0 - movil[i]).abs()).sum::<f32>() / n.max(1) as f32;
        println!("peor: {} ({:.1} px); media {medias:.1} px; alto {:.1} vs {alto_movil}", peor.0, peor.1, alto);
        assert_eq!(movil.len(), pc.len(), "la misma estructura de bloques");
    }
}
