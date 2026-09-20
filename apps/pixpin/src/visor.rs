//! **El visor limpio**: un Word, un libro o una pagina, a pantalla completa
//! y sin nada alrededor salvo el nombre.
//!
//! Es el puerto de `ui/VisorHtmlActivity.kt` del movil (v0.62-v0.66). Alli
//! lo ensena un `WebView`; aqui no hay navegador ni se va a arrastrar uno:
//! `pixpin-docs` deja el documento en bloques de texto con estilo y esto
//! los pinta con el mismo `Pintor` que el resto de la aplicacion. Se pierde
//! la maquetacion fina —las imagenes se anuncian, las tablas se leen en una
//! linea— y se gana lo que el usuario queria: **leerlo**, con la letra que
//! el elija, y que la aplicacion arranque igual de rapido que antes.
//!
//! Del movil se copia lo que importa:
//!
//! - **Solo el nombre arriba**, en una pastilla semitransparente que sale
//!   al tocar y se va sola al leer.
//! - **El marcador y el engranaje junto al nombre** (v0.66), no en una
//!   barra aparte.
//! - **Leer a gusto** (v0.65): el tamano de la letra, y marcadores con
//!   emoticono que guardan **la fraccion del documento**, no un numero de
//!   pixeles, para que sigan cayendo en el mismo parrafo al cambiar la
//!   letra. Las cuentas son puras y viven en `pixpin_docs::lectura`.
//! - **Word a PDF** (v0.63): lo que alli hacia el servicio de impresion de
//!   Android lo escribe `pixpin_docs::pdf`.
//!
//! Lo que se guarda **va junto al documento**, en un fichero hermano
//! `<nombre>.pixpin-lectura`: asi los marcadores viajan con el documento en
//! vez de quedarse en un ajuste que nadie sabe donde esta.
//!
//! Este modulo no lleva `unsafe`: todo el dibujo pasa por el pintor seguro
//! de `pixpin-render`, como el overlay.

#![forbid(unsafe_code)]
// QUITAR ESTO al enganchar el visor: mientras nadie llame a `lanzar`, todo
// el modulo es codigo muerto para el compilador y la puerta comun
// (`clippy -D warnings`) no pasaria. En cuanto el menu del chat, «Abrir
// con» o la bandeja llamen a `visor::lanzar`, esta linea sobra.
#![allow(dead_code)]

use anyhow::{Context, Result};
use pixpin_docs::documento::{Clase, TramoDoc, texto_y_tramos};
use pixpin_docs::{Documento, lectura};
use pixpin_geom::Rect;
use pixpin_render::{Color, EstiloTexto, Pintor, RectF, Superficie, Tramo};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use std::path::{Path, PathBuf};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

// Los colores del «visor limpio»: papel, no interfaz. Son los mismos que
// el modo noche del movil (`DocxAHtml.kt`, bloque `prefers-color-scheme`).
const FONDO: Color = hex(0x121316);
const TEXTO: Color = hex(0xe4e2e6);
const APAGADO: Color = hex(0x9a9aa2);
const CRISTAL: Color = hex(0x26262c);
const RAYA: Color = hex(0x3a3a42);
const DORADO: Color = hex(0xe8c06a);

/// Cuanto se lee de una vez con la rueda, en lineas.
const LINEAS_POR_MUESCA: f32 = 3.0;

/// La pastilla del nombre se va sola tras esto, como en el movil.
const MS_DE_LA_PASTILLA: u64 = 2600;

const VK_ESCAPE: u32 = 0x1B;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_UP: u32 = 0x26;
const VK_DOWN: u32 = 0x28;
const VK_G: u32 = 0x47;
const VK_M: u32 = 0x4D;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;

/// Abre el visor con ese documento, en su propio hilo.
///
/// Es la unica puerta de entrada del modulo: el hilo principal tiene que
/// seguir atendiendo atajos y gestos mientras se lee, igual que con el chat
/// o con «recibir».
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("visor".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = Recursos::nuevos().and_then(|r| abrir(&r, &textos, &ubicacion, &ruta));
            if let Err(e) = hecho {
                tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el visor");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del visor");
    }
}

/// Si el visor sabe abrir eso. Es lo que mira quien pone la entrada en el
/// menu: ofrecerse para algo que no se hace bien es peor que no ofrecerse.
pub fn se_abre(nombre: &str) -> bool {
    pixpin_docs::formato_de(nombre).is_some()
}

/// Un bloque ya medido: lo que se pinta y cuanto ocupa.
struct Colocado {
    texto: String,
    tramos: Vec<Tramo>,
    clase: Clase,
    tam: f32,
    /// Sangria a la izquierda respecto del margen.
    sangria: f32,
    y: f32,
    alto: f32,
    color: Color,
}

/// Un boton de la pastilla o del panel, con su caja para el raton.
struct Boton {
    caja: RectF,
    que: Accion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Marcador,
    Engranaje,
    QuitarMarcadores,
    GuardarPagina,
    GuardarPdf,
    AbrirCarpeta,
    /// Un punto de la barra del tamano.
    Tamano(usize),
    /// Un emoticono de la fila de elegir.
    Emoji(usize),
    /// Un marcador de la tira lateral.
    IrAlMarcador(usize),
}

struct Estado {
    doc: Documento,
    ajustes: lectura::Ajustes,
    /// Por donde va la lectura, en pixeles desde el principio.
    y: f32,
    alto_total: f32,
    /// Lo medido para el ancho y el tamano de ahora.
    colocados: Vec<Colocado>,
    /// Con que ancho y tamano se midio: si cambian, hay que volver a medir.
    medido_con: (f32, u32),
    pastilla_hasta: u64,
    panel: bool,
    eligiendo_emoji: bool,
    aviso: Option<(String, u64)>,
    /// Las cajas que el raton puede pulsar en este fotograma.
    botones: Vec<Boton>,
    raton: (f32, f32),
}

pub fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    ruta: &Path,
) -> Result<()> {
    let doc = pixpin_docs::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    // A pantalla completa de verdad: el «visor limpio» del movil tapa
    // incluso la barra de estado, y aqui la ventana ocupa el monitor.
    let marco: Rect = monitor.area;

    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("visor-titulo"))
        .context("no se pudo abrir la ventana del visor")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el visor")?;
    ventana.mostrar();
    ventana.enfocar();

    let ajustes = lectura::leer(ruta);
    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    let mut e = Estado {
        doc,
        ajustes,
        y: 0.0,
        alto_total: 0.0,
        colocados: Vec::new(),
        medido_con: (0.0, 0),
        pastilla_hasta: ahora + MS_DE_LA_PASTILLA,
        panel: false,
        eligiendo_emoji: false,
        aviso: None,
        botones: Vec::new(),
        raton: (0.0, 0.0),
    };
    // Se entra por donde se dejo. Como el sitio es una fraccion, hace falta
    // medir antes, y medir necesita un fotograma: se apunta y se aplica en
    // el primero.
    let mut ir_a = e.ajustes.sitio;

    let mut hay_que_pintar = true;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Pintar => {}
                EventoOverlay::RatonMovido(punto) => {
                    e.raton = ((punto.x - marco.x) as f32, (punto.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(punto) => {
                    e.raton = ((punto.x - marco.x) as f32, (punto.y - marco.y) as f32);
                    pulsar(&mut e, textos, ruta, ubicacion);
                }
                EventoOverlay::Rueda(muescas) => {
                    let paso = tamano_base(&e) * 1.45 * escala * LINEAS_POR_MUESCA;
                    mover(&mut e, -(muescas as f32) * paso);
                }
                EventoOverlay::Tecla { vk, .. } => match vk {
                    VK_ESCAPE => {
                        if e.panel || e.eligiendo_emoji {
                            e.panel = false;
                            e.eligiendo_emoji = false;
                        } else {
                            vivo = false;
                        }
                    }
                    VK_DOWN => {
                        let linea = tamano_base(&e) * 1.45 * escala;
                        mover(&mut e, linea);
                    }
                    VK_UP => {
                        let linea = tamano_base(&e) * 1.45 * escala;
                        mover(&mut e, -linea);
                    }
                    VK_NEXT => mover(&mut e, marco.alto as f32 * 0.9),
                    VK_PRIOR => mover(&mut e, -(marco.alto as f32) * 0.9),
                    VK_HOME => e.y = 0.0,
                    VK_END => e.y = e.alto_total,
                    VK_MAS | VK_MAS_NUM => cambiar_tamano(&mut e, true, ruta),
                    VK_MENOS | VK_MENOS_NUM => cambiar_tamano(&mut e, false, ruta),
                    VK_M => {
                        e.eligiendo_emoji = !e.eligiendo_emoji;
                        e.panel = false;
                        despertar_pastilla(&mut e);
                    }
                    VK_G => {
                        e.panel = !e.panel;
                        e.eligiendo_emoji = false;
                        despertar_pastilla(&mut e);
                    }
                    _ => despertar_pastilla(&mut e),
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
        // La pastilla se va sola y el aviso tambien: los dos hacen que haya
        // que repintar aunque nadie toque nada.
        if e.pastilla_hasta >= ahora && e.pastilla_hasta < ahora + 200 {
            hay_que_pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            hay_que_pintar = true;
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    medir(&mut e, p, marco, escala);
                    if ir_a > 0.0 {
                        e.y = (e.alto_total * ir_a).clamp(0.0, e.alto_total);
                        ir_a = -1.0;
                    }
                    pintar(&mut e, p, marco, escala, textos);
                });
                let _ = superficie.presentar();
            }
        }
        // Veinte veces por segundo: es lo que hace falta para que la
        // pastilla se desvanezca a su hora sin gastar un fotograma por
        // nada cuando no pasa nada.
        pixpin_shell::overlay::esperar_eventos(Some(50));
    }

    // Por donde se iba, para volver aqui la proxima vez.
    e.ajustes.sitio = if e.alto_total > 0.0 {
        (e.y / e.alto_total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if let Err(err) = lectura::escribir(ruta, &e.ajustes) {
        // Un documento en un sitio de solo lectura no tiene por que
        // estropear la lectura: se apunta y ya.
        tracing::info!(
            ?err,
            "no se pudieron guardar los marcadores junto al documento"
        );
    }
    Ok(())
}

fn tamano_base(e: &Estado) -> f32 {
    // 16 puntos al cien por cien, como el cuerpo de la pagina del movil.
    16.0 * e.ajustes.tamano as f32 / 100.0
}

fn despertar_pastilla(e: &mut Estado) {
    e.pastilla_hasta = pixpin_shell::entorno::ahora_utc_ms() as u64 + MS_DE_LA_PASTILLA;
}

fn mover(e: &mut Estado, cuanto: f32) {
    e.y = (e.y + cuanto).clamp(0.0, e.alto_total);
}

fn cambiar_tamano(e: &mut Estado, arriba: bool, ruta: &Path) {
    // Se guarda por donde se iba ANTES de cambiar la letra y se vuelve a
    // ese mismo punto del documento despues: si no, agrandar la letra te
    // manda a otro capitulo. Es lo que hace `ponerElTamano` en el movil.
    let donde = if e.alto_total > 0.0 {
        e.y / e.alto_total
    } else {
        0.0
    };
    e.ajustes.tamano = lectura::tamano_vecino(e.ajustes.tamano, arriba);
    e.medido_con = (0.0, 0);
    e.ajustes.sitio = donde;
    let _ = lectura::escribir(ruta, &e.ajustes);
    despertar_pastilla(e);
    // La `y` se recoloca al medir de nuevo, con el alto que salga.
    e.y = e.alto_total * donde;
}

// ---------------------------------------------------------------------------
// Medir

fn tramos_de(lista: &[TramoDoc]) -> Vec<Tramo> {
    lista
        .iter()
        .map(|t| Tramo {
            inicio: t.inicio,
            longitud: t.longitud,
            estilo: EstiloTexto {
                negrita: t.estilo.negrita,
                cursiva: t.estilo.cursiva || t.estilo.enlace,
                mono: t.estilo.mono,
            },
        })
        .collect()
}

/// Cuanto crece cada clase de bloque respecto del cuerpo, y cuanto aire
/// deja delante. Son las proporciones de la hoja de estilo del movil.
fn pinta(clase: Clase) -> (f32, f32, bool) {
    // (factor de tamano, aire delante en «emes», negrita)
    match clase {
        Clase::Titulo(1) => (1.7, 1.2, true),
        Clase::Titulo(2) => (1.4, 1.1, true),
        Clase::Titulo(3) => (1.2, 1.0, true),
        Clase::Titulo(_) => (1.05, 0.9, true),
        Clase::Cita => (1.0, 0.8, false),
        Clase::Codigo => (0.9, 0.6, false),
        Clase::Nota => (0.9, 0.5, false),
        Clase::Capitulo | Clase::Regla => (1.0, 1.6, false),
        _ => (1.0, 0.35, false),
    }
}

fn medir(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32) {
    let ancho_util = ancho_de_lectura(marco, escala);
    if e.medido_con == (ancho_util, e.ajustes.tamano) {
        return;
    }
    let base = tamano_base(e) * escala;
    let mut colocados = Vec::with_capacity(e.doc.bloques.len() + 2);
    let mut y = base * 2.0;
    let mut imagen = 0usize;

    let mut cabecera = Vec::new();
    if !e.doc.titulo.trim().is_empty() {
        cabecera.push((Clase::Titulo(1), e.doc.titulo.trim().to_string()));
    }
    if !e.doc.autor.trim().is_empty() {
        cabecera.push((Clase::Nota, e.doc.autor.trim().to_string()));
    }

    let bloques = cabecera
        .iter()
        .map(|(c, t)| (*c, t.clone(), Vec::new(), 0.0f32))
        .chain(e.doc.bloques.iter().map(|b| {
            let (texto, tramos) = texto_y_tramos(b);
            let sangria = if b.clase == Clase::Lista {
                1.2
            } else if b.clase == Clase::Cita {
                1.4
            } else {
                0.0
            };
            (b.clase, texto, tramos, sangria)
        }));

    for (clase, mut texto, tramos, sangria_em) in bloques {
        let (factor, aire, negrita) = pinta(clase);
        let tam = base * factor;
        y += base * aire;
        if clase == Clase::Regla || clase == Clase::Capitulo {
            // Una raya no mide texto: ocupa su aire y ya.
            colocados.push(Colocado {
                texto: String::new(),
                tramos: Vec::new(),
                clase,
                tam,
                sangria: 0.0,
                y,
                alto: 1.0,
                color: RAYA,
            });
            y += base * aire;
            continue;
        }
        if clase == Clase::Lista {
            texto = format!("• {texto}");
        }
        if clase == Clase::Nota && texto == pixpin_docs::documento::MARCA_IMAGEN {
            // El visor pinta texto; la imagen esta guardada y sale entera
            // en la pagina que se guarda. Se dice cual es para que el que
            // lee sepa que se esta perdiendo.
            let peso = e
                .doc
                .imagenes
                .get(imagen)
                .map(|i| i.datos.len() / 1024)
                .unwrap_or(0);
            imagen += 1;
            texto = format!("🖼 imagen ({peso} kB) — se ve al guardar la página");
        }
        if texto.trim().is_empty() {
            y += tam * 0.6;
            continue;
        }
        let sangria = sangria_em * base;
        let mut tramos = tramos_de(&tramos);
        if negrita {
            // Un titulo va entero en negrita; los tramos de dentro se
            // conservan por si traian cursiva.
            tramos.insert(
                0,
                Tramo {
                    inicio: 0,
                    longitud: texto.encode_utf16().count() as u32,
                    estilo: EstiloTexto {
                        negrita: true,
                        ..Default::default()
                    },
                },
            );
        }
        let (_, alto) = p.medir_parrafo(&texto, tam, ancho_util - sangria, &tramos);
        let color = match clase {
            Clase::Nota => APAGADO,
            Clase::Cita => APAGADO,
            _ => TEXTO,
        };
        colocados.push(Colocado {
            texto,
            tramos,
            clase,
            tam,
            sangria,
            y,
            alto,
            color,
        });
        y += alto;
    }

    e.alto_total = (y + base * 4.0 - marco.alto as f32).max(0.0);
    e.colocados = colocados;
    e.medido_con = (ancho_util, e.ajustes.tamano);
    e.y = e.y.clamp(0.0, e.alto_total);
}

/// La columna de lectura: ni tan ancha que se pierda el renglon ni tan
/// estrecha que se corte. Son las 46 «emes» de la hoja del movil.
fn ancho_de_lectura(marco: Rect, escala: f32) -> f32 {
    let tope = 860.0 * escala;
    (marco.ancho as f32 - 96.0 * escala).min(tope).max(240.0)
}

// ---------------------------------------------------------------------------
// Pintar

/// Pinta el fotograma y, de paso, **apunta donde cayo cada cosa que se
/// puede pulsar**.
///
/// Las cajas se recogen aqui y no en una tabla aparte a proposito: lo que
/// se ve y lo que se pulsa salen de las mismas cuentas, asi que no pueden
/// separarse al cambiar una de las dos.
fn pintar(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32, textos: &Catalogo) {
    let mut botones: Vec<Boton> = Vec::new();
    dibujar(e, p, marco, escala, textos, &mut botones);
    e.botones = botones;
}

fn dibujar(
    e: &Estado,
    p: &Pintor,
    marco: Rect,
    escala: f32,
    textos: &Catalogo,
    botones: &mut Vec<Boton>,
) {
    let ancho = marco.ancho as f32;
    let alto = marco.alto as f32;
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
        },
        FONDO,
    );

    let util = ancho_de_lectura(marco, escala);
    let x0 = (ancho - util) / 2.0;
    for c in &e.colocados {
        let y = c.y - e.y;
        if y + c.alto < -40.0 || y > alto + 40.0 {
            continue;
        }
        if c.clase == Clase::Regla || c.clase == Clase::Capitulo {
            p.rellenar(
                RectF {
                    x: x0,
                    y,
                    ancho: util,
                    alto: 1.0 * escala,
                },
                c.color,
            );
            continue;
        }
        p.parrafo(
            &c.texto,
            x0 + c.sangria,
            y,
            c.tam,
            util - c.sangria,
            &c.tramos,
            c.color,
        );
    }

    if e.colocados.is_empty() {
        let aviso = textos.t("visor-vacio");
        let (w, _) = p.medir_texto(&aviso, 15.0 * escala);
        p.texto(
            &aviso,
            (ancho - w) / 2.0,
            alto / 2.0,
            15.0 * escala,
            APAGADO,
        );
    }

    barra_de_avance(e, p, marco, escala);
    tira_de_marcadores(e, p, marco, escala, botones);

    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    if e.pastilla_hasta > ahora || e.panel || e.eligiendo_emoji {
        pastilla(e, p, marco, escala, botones);
    }
    if e.panel {
        panel(e, p, marco, escala, textos, botones);
    }
    if e.eligiendo_emoji {
        fila_de_emojis(p, marco, escala, textos, botones);
    }
    if let Some((texto, _)) = &e.aviso {
        let tam = 14.0 * escala;
        let (w, h) = p.medir_texto(texto, tam);
        let caja = RectF {
            x: (ancho - w) / 2.0 - 16.0 * escala,
            y: alto - 110.0 * escala,
            ancho: w + 32.0 * escala,
            alto: h + 16.0 * escala,
        };
        p.rellenar_redondeado(caja, 10.0 * escala, CRISTAL);
        p.texto(
            texto,
            caja.x + 16.0 * escala,
            caja.y + 8.0 * escala,
            tam,
            TEXTO,
        );
    }
}

/// Una raya fina a la derecha con por donde va la lectura: es lo unico que
/// el «visor limpio» deja ver siempre.
fn barra_de_avance(e: &Estado, p: &Pintor, marco: Rect, escala: f32) {
    if e.alto_total <= 0.0 {
        return;
    }
    let alto = marco.alto as f32;
    let ancho = 3.0 * escala;
    let x = marco.ancho as f32 - ancho - 4.0 * escala;
    let largo = (alto * alto / (alto + e.alto_total)).max(30.0 * escala);
    let y = (e.y / e.alto_total) * (alto - largo);
    p.rellenar_redondeado(
        RectF {
            x,
            y,
            ancho,
            alto: largo,
        },
        ancho / 2.0,
        RAYA,
    );
}

/// Los marcadores, en el lateral: un punto por marcador, en el orden del
/// documento, con su emoticono. Es la tira de `LateralDeMarcadores` del
/// movil, sin la vibracion (que en un ordenador no existe).
fn tira_de_marcadores(e: &Estado, p: &Pintor, marco: Rect, escala: f32, botones: &mut Vec<Boton>) {
    let alto = marco.alto as f32;
    let x = marco.ancho as f32 - 36.0 * escala;
    for (i, m) in e.ajustes.marcadores.iter().enumerate() {
        let y = 80.0 * escala + m.fraccion * (alto - 160.0 * escala);
        p.texto(&m.emoji, x, y, 15.0 * escala, DORADO);
        botones.push(Boton {
            caja: RectF {
                x: x - 4.0 * escala,
                y: y - 4.0 * escala,
                ancho: 34.0 * escala,
                alto: 26.0 * escala,
            },
            que: Accion::IrAlMarcador(i),
        });
    }
}

/// La pastilla del nombre, con el marcador y el engranaje al lado.
fn pastilla(e: &Estado, p: &Pintor, marco: Rect, escala: f32, botones: &mut Vec<Boton>) {
    let ancho = marco.ancho as f32;
    let tam = 15.0 * escala;
    let nombre = pixpin_docs::sin_extension(&e.doc.titulo);
    let (w, h) = p.medir_texto(&nombre, tam);
    let lado = 34.0 * escala;
    let total = w + 28.0 * escala + lado * 2.0;
    let caja = RectF {
        x: (ancho - total) / 2.0,
        y: 14.0 * escala,
        ancho: total,
        alto: h + 14.0 * escala,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, CRISTAL);
    p.texto(
        &nombre,
        caja.x + 14.0 * escala,
        caja.y + 7.0 * escala,
        tam,
        TEXTO,
    );
    // El marcador y el engranaje, **junto al nombre** (v0.66 del movil).
    let x = caja.x + 14.0 * escala + w + 8.0 * escala;
    p.texto(
        "🔖",
        x,
        caja.y + 7.0 * escala,
        tam,
        if e.eligiendo_emoji { DORADO } else { APAGADO },
    );
    p.texto(
        "⚙",
        x + lado,
        caja.y + 7.0 * escala,
        tam,
        if e.panel { DORADO } else { APAGADO },
    );
    for (i, que) in [Accion::Marcador, Accion::Engranaje]
        .into_iter()
        .enumerate()
    {
        botones.push(Boton {
            caja: RectF {
                x: x - 4.0 * escala + i as f32 * lado,
                y: caja.y,
                ancho: lado,
                alto: caja.alto,
            },
            que,
        });
    }
}

/// El engranaje: una hoja desde abajo con todo a la vista y a un toque,
/// como la del movil.
fn panel(
    e: &Estado,
    p: &Pintor,
    marco: Rect,
    escala: f32,
    textos: &Catalogo,
    botones: &mut Vec<Boton>,
) {
    let ancho = marco.ancho as f32;
    let alto = marco.alto as f32;
    let alto_panel = 220.0 * escala;
    let caja = RectF {
        x: 0.0,
        y: alto - alto_panel,
        ancho,
        alto: alto_panel,
    };
    p.rellenar(caja, CRISTAL);
    p.rellenar(
        RectF {
            x: 0.0,
            y: caja.y,
            ancho,
            alto: 1.0 * escala,
        },
        RAYA,
    );
    let tam = 14.0 * escala;
    let x = 40.0 * escala;
    p.texto(
        &textos.t("visor-tamano"),
        x,
        caja.y + 22.0 * escala,
        tam,
        TEXTO,
    );
    p.texto(
        &format!("{} %", e.ajustes.tamano),
        x + 200.0 * escala,
        caja.y + 22.0 * escala,
        tam,
        APAGADO,
    );
    // La barra de puntos: cada punto un tamano, y el elegido, mas gordo.
    let elegido = lectura::punto_del_tamano(e.ajustes.tamano);
    for (i, _) in lectura::TAMANOS.iter().enumerate() {
        let centro = (
            x + 12.0 * escala + i as f32 * 34.0 * escala,
            caja.y + 62.0 * escala,
        );
        let radio = (5.0 + i as f32 * 0.9) * escala;
        p.circulo(centro, radio, if i == elegido { DORADO } else { RAYA });
        botones.push(Boton {
            caja: RectF {
                x: centro.0 - 17.0 * escala,
                y: centro.1 - 18.0 * escala,
                ancho: 34.0 * escala,
                alto: 36.0 * escala,
            },
            que: Accion::Tamano(i),
        });
    }
    for (i, (etiqueta, que)) in filas_del_panel(e, textos).into_iter().enumerate() {
        let y = caja.y + (110.0 + i as f32 * 28.0) * escala;
        p.texto(&etiqueta, x, y, tam, TEXTO);
        botones.push(Boton {
            caja: RectF {
                x,
                y: y - 4.0 * escala,
                ancho: 340.0 * escala,
                alto: 26.0 * escala,
            },
            que,
        });
    }
    p.texto(
        &textos.t("visor-ayuda"),
        x,
        alto - 24.0 * escala,
        12.0 * escala,
        APAGADO,
    );
}

/// Lo que se puede hacer desde el engranaje, con su texto ya traducido.
fn filas_del_panel(e: &Estado, textos: &Catalogo) -> Vec<(String, Accion)> {
    let mut filas = vec![
        (textos.t("visor-guardar-pagina"), Accion::GuardarPagina),
        (textos.t("visor-guardar-pdf"), Accion::GuardarPdf),
        (textos.t("visor-abrir-carpeta"), Accion::AbrirCarpeta),
    ];
    if !e.ajustes.marcadores.is_empty() {
        filas.push((
            textos.t("visor-quitar-marcadores"),
            Accion::QuitarMarcadores,
        ));
    }
    filas
}

/// Con que emoticono se pone el marcador: una fila para elegir de un toque.
fn fila_de_emojis(
    p: &Pintor,
    marco: Rect,
    escala: f32,
    textos: &Catalogo,
    botones: &mut Vec<Boton>,
) {
    let ancho = marco.ancho as f32;
    let alto = marco.alto as f32;
    let alto_fila = 84.0 * escala;
    let caja = RectF {
        x: 0.0,
        y: alto - alto_fila,
        ancho,
        alto: alto_fila,
    };
    p.rellenar(caja, CRISTAL);
    p.texto(
        &textos.t("visor-elige-emoticono"),
        40.0 * escala,
        caja.y + 10.0 * escala,
        12.0 * escala,
        APAGADO,
    );
    for (i, emoji) in lectura::EMOJIS.iter().enumerate() {
        let (x, y) = sitio_del_emoji(i, marco, escala);
        p.texto(emoji, x, y, 22.0 * escala, TEXTO);
        botones.push(Boton {
            caja: RectF {
                x: x - 6.0 * escala,
                y: y - 6.0 * escala,
                ancho: 40.0 * escala,
                alto: 40.0 * escala,
            },
            que: Accion::Emoji(i),
        });
    }
}

fn sitio_del_emoji(i: usize, marco: Rect, escala: f32) -> (f32, f32) {
    (
        40.0 * escala + i as f32 * 44.0 * escala,
        marco.alto as f32 - 46.0 * escala,
    )
}

// ---------------------------------------------------------------------------
// Pulsar

/// Que hay bajo el raton, segun lo que se apunto al pintar.
///
/// Se mira **del ultimo al primero**: el que se dibujo encima es el que
/// recibe el clic, que es lo que espera cualquiera.
fn que_hay_debajo(e: &Estado) -> Option<Accion> {
    let (rx, ry) = e.raton;
    e.botones
        .iter()
        .rev()
        .find(|b| {
            rx >= b.caja.x
                && rx <= b.caja.x + b.caja.ancho
                && ry >= b.caja.y
                && ry <= b.caja.y + b.caja.alto
        })
        .map(|b| b.que)
}

fn pulsar(e: &mut Estado, textos: &Catalogo, ruta: &Path, ubicacion: &Ubicacion) {
    match que_hay_debajo(e) {
        Some(a) => hacer(e, a, textos, ruta, ubicacion),
        None => {
            // Un toque en el papel saca (o esconde) la pastilla, como en el
            // movil, y cierra lo que hubiera abierto.
            if e.panel || e.eligiendo_emoji {
                e.panel = false;
                e.eligiendo_emoji = false;
            } else {
                despertar_pastilla(e);
            }
        }
    }
}

fn hacer(e: &mut Estado, a: Accion, textos: &Catalogo, ruta: &Path, ubicacion: &Ubicacion) {
    match a {
        Accion::Marcador => {
            e.eligiendo_emoji = true;
            e.panel = false;
        }
        Accion::Engranaje => {
            e.panel = !e.panel;
            e.eligiendo_emoji = false;
        }
        Accion::Emoji(i) => {
            let fraccion = if e.alto_total > 0.0 {
                e.y / e.alto_total
            } else {
                0.0
            };
            let emoji = lectura::EMOJIS
                .get(i)
                .copied()
                .unwrap_or(lectura::EMOJIS[0]);
            e.ajustes.marcadores = lectura::con_marcador(
                &e.ajustes.marcadores,
                fraccion,
                emoji,
                pixpin_shell::entorno::ahora_utc_ms() as u64,
            );
            let _ = lectura::escribir(ruta, &e.ajustes);
            e.eligiendo_emoji = false;
        }
        Accion::IrAlMarcador(i) => {
            if let Some(m) = e.ajustes.marcadores.get(i) {
                e.y = (e.alto_total * m.fraccion).clamp(0.0, e.alto_total);
            }
        }
        Accion::QuitarMarcadores => {
            e.ajustes.marcadores.clear();
            let _ = lectura::escribir(ruta, &e.ajustes);
        }
        Accion::Tamano(i) => {
            let donde = if e.alto_total > 0.0 {
                e.y / e.alto_total
            } else {
                0.0
            };
            e.ajustes.tamano = lectura::TAMANOS[i.min(lectura::TAMANOS.len() - 1)];
            e.medido_con = (0.0, 0);
            e.ajustes.sitio = donde;
            let _ = lectura::escribir(ruta, &e.ajustes);
        }
        Accion::GuardarPagina => {
            let pagina = pixpin_docs::documento::a_html(&e.doc);
            avisar(
                e,
                textos,
                guardar(ubicacion, ruta, "html", pagina.as_bytes()),
            );
        }
        Accion::GuardarPdf => {
            let bytes = pixpin_docs::pdf::de_documento(&e.doc);
            avisar(e, textos, guardar(ubicacion, ruta, "pdf", &bytes));
        }
        Accion::AbrirCarpeta => {
            let _ = pixpin_shell::abrir_ubicacion(&carpeta_de_salida(ubicacion));
        }
    }
    despertar_pastilla(e);
}

/// Donde van las cosas que el visor fabrica. No se escribe al lado del
/// documento: el documento puede estar en una carpeta que no es nuestra (o
/// de solo lectura), y una copia inesperada al lado de un archivo ajeno es
/// justo lo que nadie espera.
fn carpeta_de_salida(ubicacion: &Ubicacion) -> PathBuf {
    ubicacion.raiz().join("exportado")
}

fn guardar(
    ubicacion: &Ubicacion,
    ruta: &Path,
    extension: &str,
    datos: &[u8],
) -> std::io::Result<PathBuf> {
    let carpeta = carpeta_de_salida(ubicacion);
    std::fs::create_dir_all(&carpeta)?;
    let base = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let destino = carpeta.join(format!("{base}.{extension}"));
    std::fs::write(&destino, datos)?;
    Ok(destino)
}

fn avisar(e: &mut Estado, textos: &Catalogo, hecho: std::io::Result<PathBuf>) {
    let texto = match hecho {
        Ok(destino) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("nombre", pixpin_docs::nombre(&destino));
            let _ = pixpin_shell::abrir(&destino);
            textos.t_args("visor-guardado", &args)
        }
        Err(err) => {
            tracing::warn!(?err, "no se pudo guardar lo exportado desde el visor");
            textos.t("visor-no-guardado")
        }
    };
    e.aviso = Some((texto, pixpin_shell::entorno::ahora_utc_ms() as u64 + 3500));
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_visor_solo_se_ofrece_para_lo_que_sabe_abrir() {
        assert!(se_abre("apuntes.docx"));
        assert!(se_abre("libro.epub"));
        assert!(se_abre("pagina.html"));
        assert!(se_abre("notas.md"));
        assert!(
            !se_abre("plano.pdf"),
            "el PDF lo ensena el pin, no el visor"
        );
        assert!(!se_abre("foto.png"));
        assert!(!se_abre("apuntes.doc"), "un Word antiguo no se lee");
        assert!(!se_abre(""));
    }

    #[test]
    fn el_tamano_base_crece_con_el_tanto_por_ciento() {
        let mut e = estado_de_prueba();
        e.ajustes.tamano = 100;
        assert!((tamano_base(&e) - 16.0).abs() < 0.01);
        e.ajustes.tamano = 200;
        assert!((tamano_base(&e) - 32.0).abs() < 0.01);
    }

    #[test]
    fn la_columna_de_lectura_no_se_estira_en_un_monitor_ancho() {
        let estrecho = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        let anchisimo = Rect {
            ancho: 3840,
            ..estrecho
        };
        assert!(ancho_de_lectura(estrecho, 1.0) < 800.0);
        assert_eq!(
            ancho_de_lectura(anchisimo, 1.0),
            860.0,
            "en un monitor grande la columna se queda en su tope"
        );
        assert!(
            ancho_de_lectura(
                Rect {
                    ancho: 120,
                    ..estrecho
                },
                1.0
            ) >= 240.0,
            "por estrecha que sea la ventana la columna tiene un minimo"
        );
    }

    #[test]
    fn leer_no_se_sale_del_documento() {
        let mut e = estado_de_prueba();
        e.alto_total = 500.0;
        mover(&mut e, -100.0);
        assert_eq!(e.y, 0.0, "no se sube por encima del principio");
        mover(&mut e, 9999.0);
        assert_eq!(e.y, 500.0, "no se baja por debajo del final");
    }

    fn estado_de_prueba() -> Estado {
        Estado {
            doc: Documento::default(),
            ajustes: lectura::Ajustes::default(),
            y: 0.0,
            alto_total: 0.0,
            colocados: Vec::new(),
            medido_con: (0.0, 0),
            pastilla_hasta: 0,
            panel: false,
            eligiendo_emoji: false,
            aviso: None,
            botones: Vec::new(),
            raton: (0.0, 0.0),
        }
    }
}
