//! **La burbuja de una nota rica**: el Markdown de la nota pintado entero
//! dentro de su burbuja, con su texto con formato y sus fotos en su sitio
//! (el usuario, 25-oct-2026: «como si fuera un pequeno archivo markdown
//! mostrado en la burbuja», como un mensaje de Telegram con varias fotos).
//!
//! Los bloques y el album salen de `pixpin_ui::nota_rica`, que es puro y se
//! prueba sin ventana. Aqui se miden con la fuente UNA vez (`medir`, desde
//! `medir_mensaje`) y se pintan con esas mismas medidas (`pintar`): el alto
//! de la burbuja y lo pintado no pueden discrepar.
//!
//! Las fotos salen de las vistas previas del historial, como las demas
//! fotos del chat (`fotos_del_historial` las pide con [`rutas`]); mientras
//! llegan, o si su fichero no esta en este equipo, un recuadro con el icono
//! de imagen ocupa su sitio. Nunca la ruta como texto.

use super::*;
use pixpin_ui::nota_rica as nr;

/// Lo que se separa un bloque del siguiente.
const ENTRE_BLOQUES: f32 = 6.0;
/// La sangria de cada nivel de una lista y el sitio de su vineta.
const NIVEL: f32 = 14.0;
const VINETA: f32 = 16.0;
/// La casilla de una tarea y su hueco hasta el texto.
const CASILLA: f32 = 15.0;
/// La barra de una cita y su hueco hasta el texto.
const CITA_BARRA: f32 = 3.0;
const CITA_HUECO: f32 = 8.0;
/// La letra y el relleno de un bloque de codigo.
const CODIGO_TAM: f32 = 12.0;
const CODIGO_RELLENO: f32 = 6.0;
/// El hueco entre las fotos de un album, como en Telegram.
const HUECO_ALBUM: f32 = 2.0;
/// El radio de las esquinas de cada foto.
const RADIO_FOTO: f32 = 4.0;

/// Un bloque ya medido. Las `y` van desde lo alto de la pieza.
pub(super) enum Medido {
    Texto {
        y: f32,
        /// Donde empieza el texto (la sangria de la lista o de la cita).
        x: f32,
        texto: String,
        tam: f32,
        tramos: Vec<pixpin_render::Tramo>,
        enlaces: Vec<((u32, u32), String)>,
        tachados: Vec<(u32, u32)>,
        /// Lo que va delante: la vineta o el numero, y su `x`.
        marca: Option<(String, f32)>,
        /// La casilla de una tarea: si esta hecha.
        casilla: Option<(bool, f32)>,
        cita: bool,
        /// Lo que ocupa a lo ancho, con la sangria.
        ancho: f32,
        alto: f32,
    },
    Codigo {
        y: f32,
        texto: String,
        ancho: f32,
        alto: f32,
    },
    Regla {
        y: f32,
    },
    Album {
        y: f32,
        /// Cada foto: su caja, su fichero si esta aqui y cual es de la nota
        /// (para abrirla al pulsarla).
        fotos: Vec<(nr::Caja, Option<std::path::PathBuf>, usize)>,
    },
}

/// La nota entera ya medida.
pub(super) struct Pieza {
    pub(super) bloques: Vec<Medido>,
    pub(super) ancho: f32,
    pub(super) alto: f32,
}

/// Si la burbuja de `m` se pinta como nota rica: una nota escrita (no una
/// mini-app ni una respuesta del asistente, que tiene su propio formato).
pub(super) fn es_nota(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    matches!(m.clase, None | Some(Clase::Nota))
        && m.miniapp.is_none()
        && !crate::asistente::chat::es_del_asistente(m)
}

/// Cada cuanto se vuelve a mirar en el disco una foto de una nota: si esta
/// y si cambio. Medir la conversacion entera (al cambiar el ancho, en cada
/// paso de arrastrar el borde de la ventana) preguntaria al disco por cada
/// foto de cada nota, miles de veces por segundo en un chat largo; asi, una
/// vez cada tanto, y una foto que llega con la sincronizacion se ve en la
/// siguiente medida despues de esto.
const MIRAR_CADA: std::time::Duration = std::time::Duration::from_secs(3);

/// Lo sabido de una foto de una nota: cuando se miro, su fecha (`None` si
/// no esta) y lo que mide (`None` si no se pudo leer su cabecera).
#[derive(Clone, Copy)]
struct Sabido {
    visto: std::time::Instant,
    fecha: Option<std::time::SystemTime>,
    medida: Option<(u32, u32)>,
}

thread_local! {
    /// Lo sabido de cada foto de las notas, por fichero: la cabecera se lee
    /// una vez (y otra si cambia su fecha), no en cada medida.
    static SABIDO: std::cell::RefCell<std::collections::HashMap<std::path::PathBuf, Sabido>> =
        Default::default();
}

/// Lo que se sabe de `ruta`, mirando el disco solo si hace mas de
/// [`MIRAR_CADA`] que no se miraba.
fn sabido(ruta: &std::path::Path) -> Sabido {
    let ahora = std::time::Instant::now();
    if let Some(s) = SABIDO.with(|m| m.borrow().get(ruta).copied())
        && ahora.duration_since(s.visto) < MIRAR_CADA
    {
        return s;
    }
    let fecha = std::fs::metadata(ruta)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|m| m.modified().unwrap_or(std::time::UNIX_EPOCH));
    let antes = SABIDO.with(|m| m.borrow().get(ruta).copied());
    let medida = match antes {
        Some(a) if a.fecha == fecha => a.medida,
        _ if fecha.is_none() => None,
        // Solo la cabecera: lo que mide, girado segun su EXIF como lo
        // pintara `miniaturas` (que la lee entera, fuera de aqui).
        _ => pixpin_codec::imagen::medidas(ruta)
            .inspect_err(|e| tracing::info!(?e, ruta = %ruta.display(), "foto de nota sin medidas"))
            .ok(),
    };
    let s = Sabido {
        visto: ahora,
        fecha,
        medida,
    };
    SABIDO.with(|m| m.borrow_mut().insert(ruta.to_path_buf(), s));
    s
}

/// Ancho / alto de una foto, o `NaN` si no se sabe (el album pone un 4:3).
fn proporcion(ruta: Option<&std::path::Path>) -> f32 {
    match ruta.and_then(|r| sabido(r).medida) {
        Some((w, h)) if w > 0 && h > 0 => w as f32 / h as f32,
        _ => f32::NAN,
    }
}

/// El fichero de este equipo al que lleva el enlace de una foto de la nota:
/// `pixpin:files/…` (lo normal), relativo a la carpeta del proyecto o una
/// ruta de Windows escrita a mano. Solo si esta.
///
/// Resolver una ruta portatil lee el indice de proyectos: se hace una vez
/// por enlace y se guarda; lo que se mira en cada medida es solo si el
/// fichero ya esta (una foto puede llegar con la sincronizacion despues).
///
/// **Nunca una ruta de red** (`\\equipo\carpeta\x.png`): la nota puede
/// llegar de otro aparato, y solo con pintarla Windows llamaria a ese equipo
/// y le daria las credenciales del usuario para entrar. Ni una relativa que
/// se salga de la carpeta del proyecto con `..`.
pub(super) fn ruta_de(
    raiz: &std::path::Path,
    proyecto: &str,
    enlace: &str,
) -> Option<std::path::PathBuf> {
    let enlace = enlace.trim();
    if enlace.starts_with("\\\\") || enlace.starts_with("//") {
        return None;
    }
    if std::path::Path::new(enlace).is_absolute() && !enlace.starts_with('/') {
        let ruta = std::path::PathBuf::from(enlace);
        return sabido(&ruta).fecha.is_some().then_some(ruta);
    }
    if enlace.split(['/', '\\']).any(|trozo| trozo == "..") {
        return None;
    }
    let clave = (raiz.to_path_buf(), proyecto.to_string(), enlace.to_string());
    RUTAS
        .with(|r| {
            r.borrow_mut()
                .entry(clave)
                .or_insert_with(|| pixpin_proyecto::vista::ruta_real(raiz, proyecto, enlace))
                .clone()
        })
        .filter(|p| sabido(p).fecha.is_some())
}

thread_local! {
    /// Cada enlace de foto ya resuelto a su ruta, este o no el fichero.
    static RUTAS: std::cell::RefCell<
        std::collections::HashMap<(std::path::PathBuf, String, String), Option<std::path::PathBuf>>,
    > = Default::default();
}

/// El estilo de DirectWrite de una marca de la nota.
fn tramo_de(m: &nr::Marca) -> pixpin_render::Tramo {
    pixpin_render::Tramo {
        inicio: m.inicio,
        longitud: m.largo,
        estilo: pixpin_render::EstiloTexto {
            negrita: m.negrita,
            cursiva: m.cursiva,
            mono: m.codigo,
        },
    }
}

/// Lo que mide un parrafo con sus tramos, y como poco un renglon: una
/// casilla sin texto tambien ocupa su sitio.
fn medir_texto(
    p: &Pintor,
    texto: &str,
    tam: f32,
    ancho: f32,
    tramos: &[pixpin_render::Tramo],
) -> (f32, f32) {
    let (_, renglon) = p.medir_texto("Ag", tam);
    if texto.is_empty() {
        return (0.0, renglon);
    }
    let (w, h) = p.medir_parrafo(texto, tam, ancho.max(1.0), tramos);
    (w, h.max(renglon))
}

/// Mide la nota de `m` para su burbuja, o `None` si no es una nota con
/// algo que ensenar con formato: una nota llana sigue por el camino de
/// siempre, y no cambia ni un pixel.
pub(super) fn medir(
    p: &Pintor,
    a: &Abierto,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    texto: &str,
    ancho_max: f32,
    aguja: &str,
    e: f32,
) -> Option<Pieza> {
    use pixpin_ui::historial as h;
    if !es_nota(m) || texto.is_empty() {
        return None;
    }
    let bloques = nr::bloques(texto);
    if !nr::tiene_formato(&bloques, texto) {
        return None;
    }
    let (mut ancho, mut alto) = (0.0f32, 0.0f32);
    let mut medidos = Vec::with_capacity(bloques.len());
    let mut foto = 0usize;
    for b in bloques {
        let y = if medidos.is_empty() {
            0.0
        } else {
            alto + ENTRE_BLOQUES * e
        };
        let medido = match b {
            nr::Bloque::Texto(clase, linea) => {
                medir_bloque_de_texto(p, clase, linea, y, ancho_max, aguja, e)
            }
            nr::Bloque::Adjunto(i) => {
                // Un fichero que no es una foto: su nombre con el clip.
                let nombre = if i.alt.is_empty() {
                    i.enlace.rsplit('/').next().unwrap_or_default().to_string()
                } else {
                    i.alt
                };
                let linea = nr::Linea {
                    texto: format!("📎 {nombre}"),
                    ..Default::default()
                };
                medir_bloque_de_texto(p, nr::Clase::Parrafo, linea, y, ancho_max, aguja, e)
            }
            nr::Bloque::Codigo(texto) => {
                let tramos = [mono(&texto)];
                let relleno = CODIGO_RELLENO * e;
                let (w, alto_texto) = medir_texto(
                    p,
                    &texto,
                    CODIGO_TAM * e,
                    ancho_max - 2.0 * relleno,
                    &tramos,
                );
                Medido::Codigo {
                    y,
                    texto,
                    ancho: (w + 2.0 * relleno).min(ancho_max),
                    alto: alto_texto + 2.0 * relleno,
                }
            }
            nr::Bloque::Regla => Medido::Regla { y },
            nr::Bloque::Imagenes(imagenes) => {
                let rutas: Vec<Option<std::path::PathBuf>> = imagenes
                    .iter()
                    .map(|i| ruta_de(&a.raiz, &a.ficha.id, &i.enlace))
                    .collect();
                let proporciones: Vec<f32> =
                    rutas.iter().map(|r| proporcion(r.as_deref())).collect();
                // El tope de una foto sola es el de las fotos del chat
                // (vez y media el alto de la vista, `tamano_de_vista`).
                let (cajas, _) = nr::album(
                    &proporciones,
                    ancho_max,
                    h::VISTA_ALTO as f32 * e * 1.5,
                    h::VISTA_ALTO as f32 * e,
                    HUECO_ALBUM * e,
                );
                let fotos = cajas
                    .into_iter()
                    .zip(rutas)
                    .map(|(c, r)| {
                        foto += 1;
                        (c, r, foto - 1)
                    })
                    .collect();
                Medido::Album { y, fotos }
            }
        };
        let (w, alto_bloque) = medida_de(&medido, ancho_max, e);
        ancho = ancho.max(w.min(ancho_max));
        alto = y + alto_bloque;
        medidos.push(medido);
    }
    Some(Pieza {
        bloques: medidos,
        ancho,
        alto,
    })
}

/// Todo el texto en monoespaciada.
fn mono(texto: &str) -> pixpin_render::Tramo {
    pixpin_render::Tramo {
        inicio: 0,
        longitud: texto.encode_utf16().count() as u32,
        estilo: pixpin_render::EstiloTexto {
            mono: true,
            ..Default::default()
        },
    }
}

/// Lo ancho y lo alto de un bloque ya medido.
fn medida_de(b: &Medido, ancho_max: f32, e: f32) -> (f32, f32) {
    match b {
        Medido::Texto { ancho, alto, .. } => (*ancho, *alto),
        Medido::Codigo { ancho, alto, .. } => (*ancho, *alto),
        // Una raya no pide ancho: ocupa el de la burbuja.
        Medido::Regla { .. } => (0.0, 9.0 * e),
        Medido::Album { fotos, .. } => fotos.iter().fold((0.0f32, 0.0f32), |(w, h), (c, _, _)| {
            (w.max(c.x + c.ancho).min(ancho_max), h.max(c.y + c.alto))
        }),
    }
}

/// Mide un parrafo, titulo, renglon de lista o cita.
fn medir_bloque_de_texto(
    p: &Pintor,
    clase: nr::Clase,
    linea: nr::Linea,
    y: f32,
    ancho_max: f32,
    aguja: &str,
    e: f32,
) -> Medido {
    use pixpin_ui::historial as h;
    let base = h::TEXTO_TAM * e;
    let tam = match clase {
        nr::Clase::Titulo(1) => 18.0 * e,
        nr::Clase::Titulo(2) => 16.0 * e,
        nr::Clase::Titulo(_) => 14.5 * e,
        _ => base,
    };
    let nr::Linea {
        texto,
        marcas,
        enlaces,
    } = linea;
    let mut tramos = negritas(&texto, aguja);
    tramos.extend(marcas.iter().map(tramo_de));
    if matches!(clase, nr::Clase::Titulo(_)) {
        tramos.push(pixpin_render::Tramo {
            inicio: 0,
            longitud: texto.encode_utf16().count() as u32,
            estilo: pixpin_render::EstiloTexto {
                negrita: true,
                ..Default::default()
            },
        });
    }
    let tachados: Vec<(u32, u32)> = marcas
        .iter()
        .filter(|m| m.tachado)
        .map(|m| (m.inicio, m.largo))
        .collect();
    // Los enlaces del Markdown y, como en cualquier nota, las direcciones
    // sueltas (que no caigan dentro de otro).
    let mut todos: Vec<((u32, u32), String)> = enlaces
        .into_iter()
        .map(|en| ((en.inicio, en.largo), en.url))
        .collect();
    for ((inicio, largo), url) in tarjetas::enlaces_utf16(&texto) {
        let pisa = todos
            .iter()
            .any(|((i, l), _)| inicio < i + l && *i < inicio + largo);
        if !pisa {
            todos.push(((inicio, largo), url));
        }
    }
    // La sangria y lo que va delante.
    let (x, marca, casilla, cita) = match &clase {
        nr::Clase::Vineta(n) => {
            let sangria = *n as f32 * NIVEL * e;
            (
                sangria + VINETA * e,
                Some(("•".to_string(), sangria + 3.0 * e)),
                None,
                false,
            )
        }
        nr::Clase::Numero(n, numero) => {
            let sangria = *n as f32 * NIVEL * e;
            let (w, _) = p.medir_texto(numero, base);
            (
                sangria + (w + 5.0 * e).max(VINETA * e),
                Some((numero.clone(), sangria)),
                None,
                false,
            )
        }
        nr::Clase::Casilla(n, hecha) => {
            let sangria = *n as f32 * NIVEL * e;
            (
                sangria + (CASILLA + 5.0) * e,
                None,
                Some((*hecha, sangria)),
                false,
            )
        }
        nr::Clase::Cita => ((CITA_BARRA + CITA_HUECO) * e, None, None, true),
        _ => (0.0, None, None, false),
    };
    let (w, alto) = medir_texto(p, &texto, tam, ancho_max - x, &tramos);
    Medido::Texto {
        y,
        x,
        texto,
        tam,
        tramos,
        enlaces: todos,
        tachados,
        marca,
        casilla,
        cita,
        ancho: x + w,
        alto,
    }
}

/// Las fotos de la nota que se pintan, para cargarlas antes del fotograma.
pub(super) fn rutas(pz: &Pieza) -> impl Iterator<Item = std::path::PathBuf> + '_ {
    pz.bloques.iter().flat_map(|b| match b {
        Medido::Album { fotos, .. } => fotos
            .iter()
            .filter_map(|(_, r, _)| r.clone())
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    })
}

/// Los colores con que se pinta la nota: los de su burbuja.
pub(super) struct Colores {
    pub(super) texto: Color,
    pub(super) apagado: Color,
    pub(super) enlace: Color,
    pub(super) acento: Color,
}

/// Pinta la nota con lo medido, desde `(x, y)`, y apunta lo que se pulsa:
/// los enlaces y cada foto (que se abre en un pin, como las del lector).
#[allow(clippy::too_many_arguments)]
pub(super) fn pintar(
    p: &Pintor,
    a: &Abierto,
    i: usize,
    pz: &Pieza,
    x0: f32,
    y0: f32,
    ancho: f32,
    area: Rect,
    previas: &crate::miniaturas::Miniaturas,
    c: &Colores,
    e: f32,
) {
    for b in &pz.bloques {
        match b {
            Medido::Texto {
                y,
                x,
                texto,
                tam,
                tramos,
                enlaces,
                tachados,
                marca,
                casilla,
                cita,
                alto,
                ..
            } => {
                let (tx, ty, tw) = (x0 + x, y0 + y, (ancho - x).max(1.0));
                let tramos = tramos.as_slice();
                if *cita {
                    p.rellenar(
                        RectF {
                            x: x0,
                            y: ty,
                            ancho: CITA_BARRA * e,
                            alto: *alto,
                        },
                        c.acento,
                    );
                }
                if let Some((m, mx)) = marca {
                    p.texto(m, x0 + mx, ty, h_tam(e), c.texto);
                }
                let hecha = casilla.is_some_and(|(h, _)| h);
                if let Some((hecha, cx)) = casilla {
                    let lado = CASILLA * e;
                    let (_, renglon) = p.medir_texto("Ag", *tam);
                    p.icono(
                        if *hecha {
                            &mi::CHECK_BOX
                        } else {
                            &mi::CHECK_BOX_OUTLINE_BLANK
                        },
                        RectF {
                            x: x0 + cx,
                            y: ty + (renglon - lado) / 2.0,
                            ancho: lado,
                            alto: lado,
                        },
                        c.acento,
                    );
                }
                if texto.is_empty() {
                    continue;
                }
                let sitios: Vec<(u32, u32)> = enlaces.iter().map(|(s, _)| *s).collect();
                let color = if hecha { c.apagado } else { c.texto };
                p.parrafo_con_enlaces(texto, tx, ty, *tam, tw, tramos, color, &sitios, c.enlace);
                let grosor = (1.0 * e).max(1.0);
                for (inicio, largo) in tachados {
                    for r in p.cajas_de_trozo(texto, *tam, tw, tramos, *inicio, *largo) {
                        let medio = ty + r.y + r.alto / 2.0;
                        p.linea(
                            (tx + r.x, medio),
                            (tx + r.x + r.ancho, medio),
                            grosor,
                            color,
                        );
                    }
                }
                for ((inicio, largo), url) in enlaces {
                    let k = {
                        let mut pintados = a.enlaces_pintados.borrow_mut();
                        pintados.push(url.clone());
                        pintados.len() - 1
                    };
                    let mut zonas = a.zonas.borrow_mut();
                    for r in p.cajas_de_trozo(texto, *tam, tw, tramos, *inicio, *largo) {
                        let r = Rect {
                            x: (tx + r.x).floor() as i32,
                            y: (ty + r.y).floor() as i32,
                            ancho: r.ancho.ceil().max(1.0) as u32,
                            alto: r.alto.ceil().max(1.0) as u32,
                        };
                        if let Some(r) = area.interseccion(r) {
                            zonas.push((r, Zona::EnlaceDelTexto(k)));
                        }
                    }
                }
            }
            Medido::Codigo { y, texto, alto, .. } => {
                // Del ancho de la burbuja, como un bloque de codigo de
                // cualquier lector: la caja dice donde empieza y acaba.
                let caja = RectF {
                    x: x0,
                    y: y0 + y,
                    ancho,
                    alto: *alto,
                };
                p.rellenar_redondeado(caja, 4.0 * e, con_alfa(c.texto, 0.08));
                let relleno = CODIGO_RELLENO * e;
                p.parrafo(
                    texto,
                    caja.x + relleno,
                    caja.y + relleno,
                    CODIGO_TAM * e,
                    (caja.ancho - 2.0 * relleno).max(1.0),
                    &[mono(texto)],
                    c.texto,
                );
            }
            Medido::Regla { y } => {
                p.rellenar(
                    RectF {
                        x: x0,
                        y: y0 + y + 4.0 * e,
                        ancho,
                        alto: (1.0 * e).max(1.0),
                    },
                    con_alfa(c.apagado, 0.5),
                );
            }
            Medido::Album { y, fotos } => {
                for (caja, ruta, k) in fotos {
                    let r = RectF {
                        x: x0 + caja.x,
                        y: y0 + y + caja.y,
                        ancho: caja.ancho,
                        alto: caja.alto,
                    };
                    let bitmap = ruta.as_deref().and_then(|ruta| previas.ya(ruta));
                    let recortada = p.empujar_recorte_redondeado(r, RADIO_FOTO * e);
                    match bitmap {
                        // Recortada por el centro tambien la sola: su caja ya
                        // tiene su proporcion y sale entera, y si su cabecera
                        // no se pudo leer (la caja es un 4:3) no se deforma.
                        Some((b, w, h)) => crate::miniaturas::pintar_recortado(p, b, r, w, h),
                        // Mientras llega, o si su fichero no esta aqui: su
                        // sitio con el icono de imagen, nunca la ruta.
                        None => {
                            p.rellenar(r, con_alfa(c.apagado, 0.18));
                            let lado = (24.0 * e).min(r.ancho).min(r.alto);
                            p.icono(
                                &mi::IMAGE,
                                RectF {
                                    x: r.x + (r.ancho - lado) / 2.0,
                                    y: r.y + (r.alto - lado) / 2.0,
                                    ancho: lado,
                                    alto: lado,
                                },
                                c.apagado,
                            );
                        }
                    }
                    if recortada {
                        p.soltar_recorte_redondeado();
                    }
                    if ruta.is_some() {
                        let zona = Rect {
                            x: r.x as i32,
                            y: r.y as i32,
                            ancho: r.ancho as u32,
                            alto: r.alto as u32,
                        };
                        if let Some(zona) = area.interseccion(zona) {
                            a.zonas.borrow_mut().push((zona, Zona::ImagenDeNota(i, *k)));
                        }
                    }
                }
            }
        }
    }
}

/// La letra de las vinetas y numeros: la del texto.
fn h_tam(e: f32) -> f32 {
    pixpin_ui::historial::TEXTO_TAM * e
}

/// Abre en un pin la foto `k` de la nota `i`, como al pulsar una foto en el
/// lector: es donde se ve grande, se amplia y se copia.
pub(super) fn abrir_foto(a: &Abierto, i: usize, k: usize, textos: &Catalogo) -> Efecto {
    let Some(m) = a.mensajes.get(i) else {
        return Efecto::Nada;
    };
    let bloques = nr::bloques(m.texto.trim());
    let ruta = nr::imagenes(&bloques)
        .nth(k)
        .and_then(|img| ruta_de(&a.raiz, &a.ficha.id, &img.enlace));
    match ruta {
        Some(ruta) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) => {
            Efecto::Aviso(textos.t("chat-pineado"))
        }
        Some(_) => {
            tracing::warn!("no contesta la ventana principal");
            Efecto::Nada
        }
        None => Efecto::Aviso(textos.t("chat-sin-archivo")),
    }
}

/// **Una foto pegada con texto ya escrito va DENTRO de lo escrito**: se
/// guarda en `notas/` del proyecto (donde guarda las suyas el editor de
/// notas) y en la caja queda su renglon `![foto 1](notas/…)` donde estaba la
/// rayita. Al enviar sale UNA nota con el texto y sus fotos, como un
/// mensaje de Telegram con su album. Devuelve cuantas entraron.
pub(super) fn fotos_al_borrador(
    raiz: &std::path::Path,
    a: &mut Abierto,
    fotos: &[(String, Vec<u8>)],
) -> std::io::Result<usize> {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &a.ficha.id).join("notas");
    std::fs::create_dir_all(&carpeta)?;
    let mut renglones = Vec::new();
    // Las fotos se numeran detras de las que ya lleve lo escrito.
    let ya = nr::imagenes(&nr::bloques(&a.borrador)).count();
    for (nombre, bytes) in fotos {
        // Sin repetir milisegundo: dos fotos pegadas juntas no se pisan.
        let cuando = descripcion::hora_sin_repetir();
        // Una imagen del portapapeles ya trae la hora en su nombre
        // (`pegado-<ms>.png`): con la de delante basta.
        let nombre = if nombre.starts_with("pegado-") {
            "pegado.png"
        } else {
            nombre
        };
        let fichero = format!(
            "{cuando}-{}",
            crate::notas_md::adjuntos::nombre_limpio(nombre)
        );
        std::fs::write(carpeta.join(&fichero), bytes)?;
        renglones.push(nr::renglon_de_foto(&fichero, ya + renglones.len() + 1));
    }
    if renglones.is_empty() {
        return Ok(0);
    }
    let pos = a.cursor.pos(&a.borrador);
    let (antes, despues) = a.borrador.split_at(pos);
    let s = nr::en_su_renglon(antes, despues, &renglones);
    a.cursor.escribir(&mut a.borrador, &s);
    Ok(renglones.len())
}

/// Lo escrito tal como se guarda: las fotos pegadas en la caja (`notas/…`)
/// con su ruta portatil, para que viajen con la nota.
pub(super) fn para_guardar(raiz: &std::path::Path, proyecto: &str, texto: &str) -> String {
    if !texto.contains(&format!("]({}", nr::NOTAS)) {
        return texto.to_string();
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto).join("notas");
    let portatil = pixpin_proyecto::vista::portatil_de_ruta(raiz, &carpeta);
    nr::enlaces_portatiles(texto, portatil.as_deref())
}
