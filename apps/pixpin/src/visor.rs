//! **El lector de Word, libros y paginas**: a pantalla completa, sin nada
//! alrededor salvo el nombre, y con lo que el movil fue anadiendo para
//! leer a gusto y anotar encima.
//!
//! Es el puerto de `ui/VisorHtmlActivity.kt` del movil (v0.62-v0.85). Alli
//! lo ensena un `WebView`; aqui no hay navegador ni se va a arrastrar uno:
//! `pixpin-docs` deja el documento en bloques de texto con estilo y esto
//! los pinta con el mismo `Pintor` (Direct2D + DirectWrite) que el resto de
//! la aplicacion. Se pierde la maquetacion fina —las imagenes se anuncian;
//! las tablas si van como tablas, celda a celda (`tablas`)— y se gana lo que el usuario queria:
//! **leerlo**, abrir al instante y que nada tiemble.
//!
//! Lo que se copia del movil:
//!
//! - **Solo el nombre arriba** (v0.62), en una pastilla que sale al tocar y
//!   se va sola al leer; a su lado el indice y el engranaje.
//! - **Leer a gusto** (v0.65): tamano, tipo y grosor de la letra, y volver
//!   por donde se iba. Las cuentas viven en `pixpin_docs::lectura`.
//! - **Marcadores con emoticono** (v0.65-v0.81) con el riel del lienzo: la
//!   isla arriba a la derecha y un punto por marcador.
//! - **Zoom propio** (v0.72): el aumento es del lector, no del documento; el
//!   texto y lo anotado van en la misma transformada. **Alejar hasta ver el
//!   texto con sus dos margenes** (v0.73/v0.75) y el iman que devuelve la
//!   vista a la columna.
//! - **Anotar dentro de la pagina** (v0.70, v0.85): la capa de tinta vive en
//!   las unidades del documento y se pinta en la misma pasada que el texto
//!   (ver `lector_tinta`). La primera vez que se anota se fijan la columna
//!   y la letra, como `columnaDeAnotar` del movil: si el texto volviera a
//!   partirse en otras lineas, lo anotado quedaria sobre otra palabra.
//! - **La maqueta del movil** (K16): el texto se coloca con la hoja de estilo,
//!   las letras y los cortes de renglon de la pagina del movil
//!   (`maqueta_movil`), asi que con la misma columna la tinta cae en la
//!   misma palabra en los dos aparatos.
//! - **Word a PDF** (v0.63): `pixpin_docs::pdf`.
//! - **Escuchar** (22-sep-2026): la voz de Windows lee por parrafos, con
//!   el que suena en ambar y el **marcador verde** donde se dejo
//!   (`leer_en_voz`, `pixpin_docs::voz_alta`). L o el boton de la pastilla.
//! - **Espacio a cada lado** (23-sep-2026): los mandos de abajo (−, +, el
//!   candado del lado), que viajan en la maqueta (`izq`, `der`).
//!
//! Lo que se guarda va **junto al documento**: `<nombre>.pixpin-lectura`
//! (letra, sitio, marcadores, aumento) y la carpeta
//! `<nombre>.pixpin-anotado` (la tinta).
//!
//! # Teclas y raton
//!
//! Rueda: leer. Ctrl+rueda: acercar o alejar hacia el raton. Mayus+rueda o
//! la rueda de lado: correr a los margenes. Arrastrar con el boton
//! izquierdo (o el central, tambien anotando): mover. Flechas, AvPag/RePag,
//! Espacio, Inicio/Fin. `+`/`-` tamano de la letra, `T` tipo, `B` grosor.
//! `M` marcador, Ctrl+1..9 ir al marcador, clic derecho en su punto:
//! quitarlo. `A` anotar (P lapiz, R resaltador, E goma, 1-5 color,
//! Ctrl+Z/Ctrl+Y). `I` indice, `G` ajustes, Ctrl+0 aumento normal, Esc
//! salir.
//!
//! Este modulo no lleva `unsafe`: todo el dibujo pasa por el pintor seguro
//! de `pixpin-render`.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use pixpin_docs::documento::{Clase, texto_y_tramos};
use pixpin_docs::{Documento, indice, lectura, vista};
#[cfg(test)]
use pixpin_motor2d::vector::Punto2;
use pixpin_render::icono::material;
use pixpin_render::{Color, EstiloTexto, Pintor, RectF, Superficie, Tramo};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::riel_marcas::DestinoRiel;
use std::path::{Path, PathBuf};

use crate::lector::{
    self, APAGADO, CRISTAL, DORADO, EMOJIS, FONDO, RAYA, TEXTO, ahora_ms, con_alfa, dentro,
};
use crate::compartir::documento_web;
use crate::leer_en_voz::{self, LeerEnVoz, Suceso};
use crate::lector_tinta::{self, Capa, Tinta};
use crate::overlay::Recursos;
use pixpin_docs::voz_alta;

mod maqueta_movil;
mod maqueta_vieja;
mod tablas;
pub(crate) use maqueta_movil::{Hoja, Letra};
pub(crate) use tablas::ordenes_de_celda;

/// La pastilla del nombre se va sola tras esto, como en el movil.
const MS_DE_LA_PASTILLA: u64 = 2600;
/// Tras esto sin tocar, lo anotado se escribe a disco solo.
const MS_PARA_GUARDAR: u64 = 1500;

/// La columna de lectura: ni tan ancha que se pierda el renglon ni tan
/// estrecha que se corte. Las 46 «emes» de la hoja del movil.
pub(crate) const COLUMNA_MAXIMA: f32 = 860.0;
const COLUMNA_MINIMA: f32 = 240.0;

const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_0: u32 = 0x30;
const VK_A: u32 = 0x41;
const VK_B: u32 = 0x42;
const VK_F: u32 = 0x46;
const VK_F2: u32 = 0x71;
const VK_F3: u32 = 0x72;
const VK_G: u32 = 0x47;
const VK_I: u32 = 0x49;
const VK_L: u32 = 0x4C;
const VK_M: u32 = 0x4D;
const VK_S: u32 = 0x53;
const VK_T: u32 = 0x54;
const VK_Y: u32 = 0x59;
const VK_Z: u32 = 0x5A;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;

/// Abre el lector con ese documento, en su propio hilo.
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

/// Un bloque ya medido: lo que se pinta y cuanto ocupa, en unidades del
/// documento (pixeles logicos a escala 1, con el cero en el borde de la
/// columna).
pub(crate) struct Colocado {
    pub(crate) texto: String,
    pub(crate) tramos: Vec<Tramo>,
    pub(crate) clase: Clase,
    pub(crate) tam: f32,
    /// Sangria a la izquierda respecto del borde de la columna.
    pub(crate) sangria: f32,
    /// El ancho de la caja del texto: la columna menos la sangria, o el
    /// hueco de dentro de su celda en una tabla.
    pub(crate) ancho: f32,
    pub(crate) y: f32,
    pub(crate) alto: f32,
    pub(crate) color: Color,
    /// El bloque del documento del que sale (la cabecera no tiene).
    pub(crate) bloque: Option<usize>,
    /// En una tabla, la caja de su celda y sus rayas (`tablas`).
    pub(crate) caja: Option<tablas::Caja>,
    /// La letra del bloque, la del movil (`maqueta_movil`): con otra, el
    /// texto se parte en otros renglones que los que se midieron.
    pub(crate) letra: Letra,
    /// Una imagen: su hueco con un recuadro y lo que es.
    pub(crate) recuadro: bool,
    /// Donde empieza y acaba cada renglon, como se partio al medir (las
    /// reglas del navegador, `pixpin_render::lectura::partir`): se pinta
    /// por ahi y no por donde partiria DirectWrite.
    pub(crate) renglones: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Indice,
    Engranaje,
    QuitarMarcadores,
    QuitarTinta,
    GuardarPagina,
    GuardarPdf,
    /// Escuchar en voz alta (la voz de Windows, `leer_en_voz`).
    Escuchar,
    /// Un boton de la barra de escuchar.
    Voz(leer_en_voz::BotonVoz),
    /// Un boton de los mandos de los lados (espacio y candado).
    Lado(lector::BotonLado),
    /// La hoja de compartir de toda la aplicacion, con este documento.
    Compartir,
    AbrirCarpeta,
    /// Un punto de la barra del tamano.
    Tamano(usize),
    Tipo(u8),
    Grosor(u8),
    /// Una entrada del indice.
    IrA(usize),
    /// Pulsar el nombre de la pastilla: cambiarlo (D5).
    Renombrar,
    /// Un boton de la caja de buscar (D9).
    Buscar(crate::buscador::Boton),
}

#[derive(Debug, Clone, Copy)]
enum Arrastre {
    /// Moviendo el documento con la mano. `movido` distingue un clic de un
    /// arrastre: un clic en el papel saca la pastilla.
    Mover {
        desde: (f32, f32),
        vista: (f32, f32),
        movido: bool,
    },
    /// Anotando: el gesto es de la tinta (`lector_tinta::Tinta`) hasta soltar.
    Trazo,
}

/// Lo que mide la ventana y a que escala.
#[derive(Debug, Clone, Copy)]
struct Marco {
    ancho: f32,
    alto: f32,
    /// Pixeles por pixel logico (1,25 a 125 %).
    e: f32,
    escala_por_cien: u32,
    /// La ventana en el escritorio: los eventos llegan en esas coordenadas y
    /// la barra y el panel de la tinta se colocan en ellas.
    area: pixpin_geom::Rect,
}

struct Estado {
    doc: Documento,
    indice: Vec<indice::Entrada>,
    ajustes: lectura::Ajustes,
    /// La esquina de arriba a la izquierda de la ventana, en unidades del
    /// documento.
    x: f32,
    y: f32,
    zoom: f32,
    /// Lo que mide el documento entero de alto, en unidades.
    alto_doc: f32,
    /// El ancho de la columna de texto, en unidades.
    columna: f32,
    colocados: Vec<Colocado>,
    /// Con que columna y letra se midio: si cambian, hay que volver a medir.
    medido_con: Option<(u32, u32, u8, u8)>,
    pastilla_hasta: u64,
    panel: bool,
    viendo_indice: bool,
    /// Lo corrido de la lista del indice, en pixeles.
    corrido_indice: f32,
    poniendo_marca: bool,
    encima: DestinoRiel,
    aviso: Option<(String, u64)>,
    /// Las cajas que el raton puede pulsar en este fotograma.
    botones: Vec<(RectF, Accion)>,
    raton: (f32, f32),
    anotando: bool,
    capa: Capa,
    /// Las herramientas de dibujo del lienzo, para anotar (`lector_tinta`).
    tinta: Tinta,
    ultimo_trazo: u64,
    arrastre: Option<Arrastre>,
    /// La esquina a lo ancho al empezar a correr de lado: el iman del
    /// centro compara con ella. Con la hora del ultimo giro.
    lateral: Option<(f32, u64)>,
    /// A donde se va la vista a lo ancho (el iman), poco a poco.
    x_meta: Option<f32>,
    /// Buscar dentro (D9): la caja de Ctrl+F y donde cae cada coincidencia.
    hallar: Hallar,
    /// El nombre de la pastilla y, si se esta cambiando, lo escrito (D5).
    nombre: crate::renombrar_doc::Pastilla,
    /// Con que hoja de estilo del movil se coloca (K16).
    hoja: Hoja,
    /// La tinta es de la maqueta de antes de K16 y hay que llevarla a la
    /// nueva al medir por primera vez.
    tinta_de_antes: bool,
    /// Algo cambio que tiene que ir al disco ya (la tinta mudada).
    guardar_ya: bool,
    /// **Escuchando** (la barra de abajo): la voz que lee el documento.
    voz: Option<LeerEnVoz>,
}

pub fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    ruta: &Path,
) -> Result<()> {
    // Cuenta como abierta para los grupos de ventanas (H9) mientras viva.
    let _grupo = crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lector {
        ruta: ruta.to_path_buf(),
    });
    let doc = pixpin_docs::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    // A pantalla completa de verdad: el «visor limpio» del movil tapa
    // incluso la barra de estado, y aqui la ventana ocupa el monitor.
    let area = monitor.area;
    let marco = Marco {
        ancho: area.ancho as f32,
        alto: area.alto as f32,
        e: monitor.escala_por_cien as f32 / 100.0,
        escala_por_cien: monitor.escala_por_cien,
        area,
    };

    let ventana = VentanaOverlay::nueva_normal(area, &textos.t("visor-titulo"))
        .context("no se pudo abrir la ventana del visor")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), area.ancho, area.alto)
        .context("sin superficie para el visor")?;
    ventana.mostrar();
    ventana.enfocar();
    // Todos los puntos del trazo y la presion del lapiz, como en el lienzo.
    ventana.pedir_entrada_fina();

    // De un adjunto del chat, lo que viaja con su mensaje (v0.96).
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    // La tinta que ya estaba en el mensaje la escribio el movil (o el PC
    // despues de K16) en la maqueta del movil; la de junto al documento con
    // un fichero de antes, en la vieja del PC.
    let del_mensaje = crate::anotado_del_adjunto::tinta_del_mensaje(ruta).is_some_and(|t| t.is_file());
    let capa = crate::anotado_del_adjunto::leer_capa(ruta);
    let tinta_de_antes = ajustes.de_antes && ajustes.letra_fijada() && !del_mensaje && !capa.vacia();
    let mut e = Estado {
        indice: indice::de(&doc),
        doc,
        zoom: ajustes.zoom,
        ajustes,
        x: 0.0,
        y: 0.0,
        alto_doc: 0.0,
        columna: 0.0,
        colocados: Vec::new(),
        medido_con: None,
        pastilla_hasta: ahora_ms() + MS_DE_LA_PASTILLA,
        panel: false,
        viendo_indice: false,
        corrido_indice: 0.0,
        poniendo_marca: false,
        encima: DestinoRiel::Fuera,
        aviso: None,
        botones: Vec::new(),
        raton: (0.0, 0.0),
        anotando: false,
        capa,
        tinta: Tinta::nueva(),
        ultimo_trazo: 0,
        arrastre: None,
        lateral: None,
        x_meta: None,
        hallar: Hallar::default(),
        nombre: crate::renombrar_doc::Pastilla::de(ubicacion.raiz(), ruta),
        hoja: Hoja::de(ruta),
        tinta_de_antes,
        guardar_ya: false,
        voz: None,
    };
    // Se entra por donde se dejo. Como el sitio es una fraccion, hace falta
    // medir antes, y medir necesita un fotograma: se apunta y se aplica en
    // el primero.
    let mut ir_a = Some(e.ajustes.sitio);

    let mut hay_que_pintar = true;
    let mut vivo = true;
    // La ruta puede cambiar al renombrar un documento suelto (D5): cada
    // vuelta trabaja con la de ahora.
    let mut ruta_viva = ruta.to_path_buf();
    let mut ruta_nueva: Option<PathBuf> = None;
    while vivo {
        let ruta_de_la_vuelta = ruta_viva.clone();
        let ruta: &Path = &ruta_de_la_vuelta;
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
            // **Anotando, la tinta primero**: son las herramientas del lienzo
            // (`lector_tinta::Tinta`) y responden igual que alli. El clic
            // mira antes el riel y los botones del lector (ver
            // `el_lector_primero`), Escape solo es de la tinta si tiene algo
            // que soltar, y el raton sigue llegando tambien al riel.
            if e.anotando {
                let para_la_tinta = match evento {
                    EventoOverlay::RatonMovido(_)
                    | EventoOverlay::Muestra(_)
                    | EventoOverlay::BotonSoltado(_) => true,
                    EventoOverlay::Caracter(_) => !escribiendo(&e),
                    EventoOverlay::BotonPulsado(p) => {
                        e.raton = local(p);
                        !el_lector_primero(&e, marco)
                    }
                    EventoOverlay::Tecla {
                        vk: VK_S,
                        shift: true,
                        ctrl: true,
                        ..
                    } => false,
                    // Con la caja de buscar abierta o cambiando el nombre, lo
                    // tecleado es de ellas; Ctrl+F y F3 tambien.
                    EventoOverlay::Tecla { vk, ctrl, .. } => {
                        !escribiendo(&e)
                            && !(ctrl && vk == VK_F)
                            && vk != VK_F3
                            && vk != VK_F2
                            && (vk != VK_ESCAPE || e.tinta.quiere_escape())
                    }
                    _ => false,
                };
                if para_la_tinta {
                    let h = a_la_tinta(&mut e, &evento, marco);
                    if let Some(c) = h.cursor {
                        ventana.poner_cursor(crate::dibujo::teclas::forma_de(c));
                    }
                    if h.cambio {
                        e.ultimo_trazo = ahora_ms();
                    }
                    if h.arrastra {
                        e.arrastre = Some(Arrastre::Trazo);
                        ventana.capturar_raton();
                    }
                    if matches!(evento, EventoOverlay::BotonSoltado(_))
                        && matches!(e.arrastre, Some(Arrastre::Trazo))
                    {
                        e.arrastre = None;
                        e.ultimo_trazo = ahora_ms();
                        ventana.soltar_raton();
                    }
                    if h.salir {
                        alternar_anotar(&mut e, ruta, marco);
                    }
                    if let EventoOverlay::RatonMovido(p) = evento {
                        // El riel resalta lo de debajo tambien anotando.
                        e.raton = local(p);
                        mover_raton(&mut e, marco);
                    }
                    if h.consumido || h.salir {
                        continue;
                    }
                }
            }
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    e.raton = local(p);
                    mover_raton(&mut e, marco);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.raton = local(p);
                    if pulsar(&mut e, textos, ruta, ubicacion, marco) {
                        ventana.capturar_raton();
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    e.raton = local(p);
                    soltar(&mut e, marco);
                    ventana.soltar_raton();
                }
                EventoOverlay::BotonCentralPulsado(p) => {
                    e.raton = local(p);
                    e.arrastre = Some(Arrastre::Mover {
                        desde: e.raton,
                        vista: (e.x, e.y),
                        movido: true,
                    });
                    e.lateral = Some((e.x, ahora_ms()));
                    ventana.capturar_raton();
                }
                EventoOverlay::BotonCentralSoltado(_) => {
                    e.arrastre = None;
                    ventana.soltar_raton();
                    fin_de_lateral(&mut e, marco);
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    e.raton = local(p);
                    pulsar_derecho(&mut e, ruta, marco);
                }
                EventoOverlay::Rueda(delta) => rueda(&mut e, delta, marco),
                EventoOverlay::RuedaHorizontal(delta) => {
                    lado(&mut e, lector::muescas(delta) * 60.0, marco);
                }
                // Ctrl+Mayus+S: compartir, el atajo del editor. Aqui y no en
                // `tecla`, que no tiene donde viven los datos.
                EventoOverlay::Tecla {
                    vk: VK_S,
                    shift: true,
                    ctrl: true,
                    alt: false,
                } => hacer(&mut e, Accion::Compartir, textos, ruta, ubicacion, marco),
                EventoOverlay::Tecla {
                    vk,
                    shift,
                    ctrl,
                    alt,
                } => {
                    let (suya, nueva) = tecla_de_las_cajas(&mut e, vk, shift, ctrl, textos, ruta, ubicacion);
                    if nueva.is_some() {
                        ruta_nueva = nueva;
                    }
                    if !suya && !tecla(&mut e, vk, shift, ctrl, alt, textos, ruta, marco) {
                        vivo = false;
                    }
                }
                EventoOverlay::Caracter(c) => {
                    if !caracter_de_las_cajas(&mut e, c) {
                        caracter(&mut e, c, ruta);
                    }
                }
                _ => {}
            }
        }
        if let Some(nueva) = ruta_nueva.take() {
            ruta_viva = nueva;
        }
        if !vivo {
            break;
        }

        // La voz: el trozo siguiente, el parrafo siguiente o el final.
        if let Some(suceso) = e.voz.as_mut().and_then(|v| v.vuelta()) {
            match suceso {
                Suceso::Parrafo(i) => al_sonar(&mut e, i, ruta, marco),
                // Acabado el documento, el verde se quita: la proxima vez se
                // empieza por lo que se este viendo (`alAcabarElDocumento`).
                Suceso::Acabado => {
                    e.ajustes.voz = None;
                    guardar_ajustes(&e, ruta);
                }
            }
            hay_que_pintar = true;
        }

        let ahora = ahora_ms();
        // La pastilla se va sola y el aviso tambien: los dos hacen que haya
        // que repintar aunque nadie toque nada.
        if e.pastilla_hasta >= ahora && e.pastilla_hasta < ahora + 200 {
            hay_que_pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            hay_que_pintar = true;
        }
        // El iman lleva la vista a la columna poco a poco: un salto seco no
        // dice adonde se ha ido.
        if let Some(meta) = e.x_meta {
            let falta = meta - e.x;
            if falta.abs() * e.zoom * marco.e < 0.5 {
                e.x = meta;
                e.x_meta = None;
            } else {
                e.x += falta * 0.25;
            }
            hay_que_pintar = true;
        }
        // Lo que se corre de lado con la rueda no tiene «soltar»: se da por
        // acabado cuando la rueda para un momento.
        if e.arrastre.is_none()
            && e.lateral.is_some_and(|(_, cuando)| ahora > cuando + 250)
        {
            fin_de_lateral(&mut e, marco);
            hay_que_pintar = true;
        }
        // El gesto de pararse: el trazo quieto se convierte en figura.
        let zoom_px = px(&e, marco);
        if e.anotando && e.tinta.forma_rapida(&mut e.capa, zoom_px) {
            e.ultimo_trazo = ahora;
            hay_que_pintar = true;
        }
        if e.capa.sucia && !e.tinta.trazando() && ahora > e.ultimo_trazo + MS_PARA_GUARDAR {
            guardar_capa(&mut e, ruta);
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    medir(&mut e, p, marco);
                    if e.hallar.saltar {
                        saltar_a_la_actual(&mut e, p, marco);
                    }
                    if let Some(f) = ir_a.take() {
                        e.y = e.alto_doc * f;
                    }
                    acotar(&mut e, marco);
                    pintar(&mut e, p, marco, textos);
                });
                let _ = superficie.presentar();
            }
            if e.guardar_ya {
                e.guardar_ya = false;
                guardar_capa(&mut e, ruta);
                guardar_ajustes(&e, ruta);
            }
        }
        // Veinte veces por segundo en reposo (la pastilla se desvanece a su
        // hora sin gastar nada), a sesenta mientras el iman se mueve.
        let tope = if e.x_meta.is_some() { 16 } else { 50 };
        // Con una pausa de forma rapida en marcha no llegan eventos: hay que
        // despertarse cuando se cumpla.
        let tope = e.tinta.mano.tope_forma_ms().map_or(tope, |t| t.min(tope));
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }

    // Por donde se iba, para volver aqui la proxima vez. Con el nombre que
    // tenga ahora: si se cambio (D5), sus cosas ya estan en el nuevo.
    let ruta: &Path = &ruta_viva;
    // La voz se calla con el lector (el verde ya esta apuntado).
    e.voz = None;
    e.ajustes.sitio = fraccion(&e);
    e.ajustes.zoom = e.zoom;
    guardar_ajustes(&e, ruta);
    guardar_capa(&mut e, ruta);
    dejar_medidas(&e, ruta);
    Ok(())
}

// ---------------------------------------------------------------------------
// Medidas

pub(crate) fn tamano_base(a: &lectura::Ajustes) -> f32 {
    // 16 puntos al cien por cien, como el cuerpo de la pagina del movil.
    16.0 * a.tamano as f32 / 100.0
}

fn renglon(e: &Estado) -> f32 {
    tamano_base(&e.ajustes) * 1.45
}

/// Pixeles de pantalla por unidad del documento.
fn px(e: &Estado, m: Marco) -> f32 {
    m.e * e.zoom
}

/// La columna que toca: la fijada si ya se anoto; si no, la que cabe.
fn columna_para(e: &Estado, m: Marco) -> f32 {
    if e.ajustes.letra_fijada() {
        return e.ajustes.columna as f32;
    }
    columna_que_cabe(m)
}

fn columna_que_cabe(m: Marco) -> f32 {
    (m.ancho / m.e - 96.0).clamp(COLUMNA_MINIMA, COLUMNA_MAXIMA)
}

/// De donde a donde va el documento a lo ancho, en unidades. Con tinta (o
/// anotando) se abren los dos margenes de dos tercios de columna.
fn limites(e: &Estado) -> (f32, f32) {
    // Una tabla mas ancha que la columna se despliega hacia el margen
    // derecho (§2.8): la vista tiene que poder llegar a verla entera.
    let tablas = e
        .colocados
        .iter()
        .filter_map(|c| c.caja.map(|k| k.x + k.ancho))
        .fold(e.columna, f32::max);
    let (izq, der) = if e.ajustes.letra_fijada() || e.anotando {
        let (i, d) = lados(e);
        (-i, e.columna + d)
    } else {
        (0.0, e.columna)
    };
    (izq, der.max(tablas))
}

/// **El espacio en blanco de cada lado de la columna**, en unidades: el
/// puesto con los mandos de abajo (`espacioIzq`/`espacioDer` del movil, que
/// viajan en la maqueta) o, si nunca se toco, los dos tercios de siempre.
fn lados(e: &Estado) -> (f32, f32) {
    match e.ajustes.lados {
        Some((i, d)) if e.ajustes.letra_fijada() => (i as f32, d as f32),
        _ => {
            let m = vista::margen_de(e.columna);
            (m, m)
        }
    }
}

fn fraccion(e: &Estado) -> f32 {
    if e.alto_doc > 0.0 {
        (e.y / e.alto_doc).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Deja la vista dentro del documento y el aumento en sus topes.
fn acotar(e: &mut Estado, m: Marco) {
    let (izq, der) = limites(e);
    let minimo = vista::zoom_minimo(der - izq, m.ancho, m.e);
    e.zoom = e.zoom.clamp(minimo, vista::ZOOM_MAXIMO);
    let s = px(e, m);
    // Un poco de aire al final: la ultima linea no se queda pegada abajo.
    let alto = e.alto_doc + renglon(e) * 4.0;
    let (x, y) = vista::dentro(e.x, e.y, izq, der, alto, m.ancho / s, m.alto / s);
    e.x = x;
    e.y = y;
}

// ---------------------------------------------------------------------------
// Medir

/// Cuanto crece cada clase de bloque respecto del cuerpo, y cuanto aire
/// deja delante. Son las proporciones de la hoja de estilo del movil.
pub(crate) fn pinta(clase: Clase) -> (f32, f32, bool) {
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

fn medir(e: &mut Estado, p: &Pintor, m: Marco) {
    let columna = columna_para(e, m);
    let clave = (
        columna.to_bits(),
        e.ajustes.tamano,
        e.ajustes.tipo,
        e.ajustes.grosor,
    );
    if e.medido_con == Some(clave) {
        return;
    }
    // Se vuelve al mismo punto del documento, no al mismo pixel: si no,
    // agrandar la letra te manda a otro capitulo (`ponerElTamano`).
    let donde = fraccion(e);
    let habia = e.alto_doc > 0.0;

    // La misma disposicion que luego se pinta: la letra del movil, sus
    // pesos y su interlineado (`pixpin_render::lectura`).
    let mide = |texto: &str, tam: f32, ancho: f32, tramos: &[Tramo], letra: &Letra| {
        p.medir_de_lectura(texto, tam, ancho, tramos, &letra.para_pintar())
    };
    let (colocados, alto_doc) = colocar(&e.doc, &e.ajustes, columna, e.hoja, &mide);
    // La tinta de antes de K16, a la maqueta nueva (una vez).
    if e.tinta_de_antes {
        e.tinta_de_antes = false;
        mudar_tinta_de_antes(e, p, columna, &colocados);
    }
    e.alto_doc = alto_doc;
    e.colocados = colocados;
    e.columna = columna;
    e.medido_con = Some(clave);
    // Las marcas de lo encontrado van en las unidades de la medida: con
    // otra letra o columna caen en otro sitio.
    e.hallar.cajas.clear();
    if habia {
        e.y = e.alto_doc * donde;
    }
}

/// **La tinta hecha con la maqueta de antes de K16, a la del movil**: cada
/// trazo al mismo sitio de su bloque (`maqueta_vieja::mudanza`). Solo la
/// que viene de junto al documento con un fichero de la version 1: la del
/// mensaje ya la escribe el movil en su maqueta.
fn mudar_tinta_de_antes(e: &mut Estado, p: &Pintor, columna: f32, nuevos: &[Colocado]) {
    if e.capa.vacia() {
        return;
    }
    let mide_viejo = |texto: &str, tam: f32, ancho: f32, tramos: &[Tramo]| {
        if tramos.is_empty() {
            p.medir_texto_ajustado(texto, tam, ancho)
        } else {
            p.medir_parrafo(texto, tam, ancho, tramos)
        }
    };
    let (viejos, _) = maqueta_vieja::colocar(&e.doc, &e.ajustes, columna, &mide_viejo);
    let bloques = bloques_de(nuevos);
    let caja = maqueta_movil::caja_de_texto(e.hoja, &e.ajustes, columna);
    let mut movidos = 0usize;
    for el in e.capa.escena.elementos.iter_mut().filter(|x| !x.borrado) {
        let (x0, y0, x1, y1) = el.caja();
        let (dx, dy) = maqueta_vieja::mudanza(((x0 + x1) / 2.0, (y0 + y1) / 2.0), &viejos, &bloques, columna, caja);
        if dx != 0.0 || dy != 0.0 {
            // Con sus puntos (un trazo los guarda donde estan) y subiendo la
            // version: esto si es un cambio que tiene que viajar.
            el.mover(dx, dy);
            movidos += 1;
        }
    }
    tracing::info!(movidos, "tinta de antes de K16 llevada a la maqueta del movil");
    e.capa.sucia |= movidos > 0;
    e.ajustes.de_antes = false;
    // Enseguida al disco, lo movido y el fichero ya nuevo: si no, la
    // proxima vez se moveria otra vez.
    e.guardar_ya = true;
}

/// Donde empieza y cuanto mide cada bloque colocado: el primero y el ultimo
/// de sus trozos (una fila de tabla tiene varias celdas y parrafos).
fn bloques_de(colocados: &[Colocado]) -> Vec<(usize, f32, f32)> {
    let mut v: Vec<(usize, f32, f32)> = Vec::new();
    for c in colocados {
        let Some(b) = c.bloque else { continue };
        let (y, fin) = match &c.caja {
            Some(k) => (k.y, k.y + k.alto),
            None => (c.y, c.y + c.alto),
        };
        match v.iter_mut().find(|x| x.0 == b) {
            Some(x) => {
                let abajo = (x.1 + x.2).max(fin);
                x.1 = x.1.min(y);
                x.2 = abajo - x.1;
            }
            None => v.push((b, y, fin - y)),
        }
    }
    v
}

/// **Coloca el documento** en una columna de `columna` unidades, **como el
/// movil** (`maqueta_movil`, K16): cada bloque con su letra, su sitio y su
/// altura, y el alto de la pagina entera.
///
/// Aparte de `medir` para que lo que se comparte (`compartir.rs`) salga con
/// la misma disposicion que la pantalla: lo anotado va en estas unidades, y
/// otra cuenta lo dejaria encima de otras palabras. `mide` da lo que ocupa
/// un parrafo a un ancho con su letra: aqui el `Pintor`; alli el motor, que
/// mide igual fuera de un fotograma.
pub(crate) fn colocar(
    doc: &Documento,
    ajustes: &lectura::Ajustes,
    columna: f32,
    hoja: Hoja,
    mide: &maqueta_movil::Mide<'_>,
) -> (Vec<Colocado>, f32) {
    maqueta_movil::colocar(doc, ajustes, columna, hoja, mide)
}

/// La altura a la que empieza el bloque `i` del documento.
fn y_del_bloque(e: &Estado, i: usize) -> f32 {
    e.colocados
        .iter()
        .find(|c| c.bloque.is_some_and(|b| b >= i))
        .map(|c| c.y)
        .unwrap_or(0.0)
}

/// El bloque que hay arriba de la ventana.
fn bloque_arriba(e: &Estado) -> usize {
    let despues = e.colocados.partition_point(|c| c.y <= e.y + 1.0);
    e.colocados[..despues]
        .iter()
        .rev()
        .find_map(|c| c.bloque)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Pintar

/// Pinta el fotograma y, de paso, **apunta donde cayo cada cosa que se
/// puede pulsar**: lo que se ve y lo que se pulsa salen de las mismas
/// cuentas, asi que no pueden separarse.
fn pintar(e: &mut Estado, p: &Pintor, m: Marco, textos: &Catalogo) {
    let mut botones: Vec<(RectF, Accion)> = Vec::new();
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: m.ancho,
            alto: m.alto,
        },
        FONDO,
    );

    // El documento y lo anotado, en la misma transformada.
    let s = px(e, m);
    p.poner_vista((0.0, 0.0), s, (-e.x * s, -e.y * s));
    let vista_doc = (e.x, e.y, e.x + m.ancho / s, e.y + m.alto / s);
    if e.anotando || e.ajustes.letra_fijada() {
        // Los bordes de la columna, apenas: anotando se ve donde acaba el
        // texto y empieza el margen.
        let linea = 1.0 / s;
        for x in [-8.0, e.columna + 8.0] {
            p.rellenar(
                RectF {
                    x,
                    y: vista_doc.1,
                    ancho: linea,
                    alto: vista_doc.3 - vista_doc.1,
                },
                con_alfa(RAYA, if e.anotando { 0.8 } else { 0.35 }),
            );
        }
    }
    // Lo encontrado, debajo del texto: se lee por encima.
    marcas_de_busqueda(e, p, vista_doc);
    // **Lo que suena**, en ambar por debajo del texto (`.pixpin-leyendo`).
    if let Some(c) = colocado_que_suena(e) {
        let aire = 3.0;
        p.rellenar_redondeado(
            RectF { x: c.sangria - aire, y: c.y - aire, ancho: c.ancho + 2.0 * aire, alto: c.alto + 2.0 * aire },
            aire,
            leer_en_voz::AMBAR_LEYENDO,
        );
    }
    let margen = 40.0 / s;
    for c in &e.colocados {
        if c.y + c.alto < vista_doc.1 - margen || c.y > vista_doc.3 + margen {
            continue;
        }
        if c.clase == Clase::Regla || c.clase == Clase::Capitulo {
            p.rellenar(
                RectF {
                    x: c.sangria,
                    y: c.y,
                    ancho: c.ancho,
                    alto: c.alto.max(1.0 / s),
                },
                c.color,
            );
            continue;
        }
        if c.recuadro {
            // El hueco de una imagen, a su tamano, y que imagen es.
            let r = RectF { x: c.sangria, y: c.y, ancho: c.ancho, alto: c.alto };
            p.rellenar(r, con_alfa(RAYA, 0.18));
            let linea = 1.0 / s;
            for raya in [
                RectF { alto: linea, ..r },
                RectF { y: r.y + r.alto - linea, alto: linea, ..r },
                RectF { ancho: linea, ..r },
                RectF { x: r.x + r.ancho - linea, ancho: linea, ..r },
            ] {
                p.rellenar(raya, con_alfa(RAYA, 0.8));
            }
            let dentro = (c.ancho - 2.0 * c.tam).max(1.0);
            p.parrafo_de_lectura(&c.texto, c.sangria + c.tam, c.y + c.tam * 0.6, c.tam, dentro, &[], &c.letra.para_pintar(), &[], c.color);
            continue;
        }
        if let Some(k) = &c.caja {
            // La celda: su fondo y su raya de un pixel, de cerca y de lejos.
            tablas::pintar_caja(p, c, k, 1.0 / s);
            if c.texto.is_empty() {
                continue;
            }
        }
        if c.texto.is_empty() {
            continue;
        }
        // Con la letra y el interlineado con que se midio: la del movil.
        p.parrafo_de_lectura(&c.texto, c.sangria, c.y, c.tam, c.ancho, &c.tramos, &c.letra.para_pintar(), &c.renglones, c.color);
    }
    // La tinta, en la misma pasada y con la misma transformada que el
    // texto: no puede quedarse atras al desplazar ni al acercar. Sobre el
    // papel oscuro del lector la tinta oscura se aclara (`dibujo::tema`).
    let papel = lector_tinta::papel(FONDO);
    e.tinta.pintar_capa(p, 0, &e.capa, vista_doc, s, papel, e.anotando);
    // Vuelta a pixeles de ventana para la interfaz.
    p.desplazar(0.0, 0.0);

    if e.colocados.is_empty() {
        let aviso = textos.t("visor-vacio");
        let (w, _) = p.medir_texto(&aviso, 15.0 * m.e);
        p.texto(&aviso, (m.ancho - w) / 2.0, m.alto / 2.0, 15.0 * m.e, APAGADO);
    }

    barra_de_avance(e, p, m);
    // Los del usuario y, entre ellos, el verde de donde se dejo de escuchar.
    let lista = lista_del_riel(e);
    let emojis: Vec<&str> = lista.iter().map(|x| x.emoji.as_str()).collect();
    let riel = lector::riel(
        m.ancho,
        m.alto,
        m.escala_por_cien,
        emojis.len(),
        e.poniendo_marca,
    );
    lector::pintar_riel(
        p,
        &riel,
        &emojis,
        e.encima,
        e.poniendo_marca,
        e.anotando,
        &textos.t("marca-elige"),
    );

    if e.anotando {
        // La barra y el panel del lienzo (`lector_tinta::Tinta`).
        e.tinta.pintar_interfaz(p, Some(&e.capa), m.area, m.escala_por_cien);
    } else if e.nombre.editando.is_some()
        || (!e.hallar.caja.abierto && (e.pastilla_hasta > ahora_ms() || e.panel || e.viendo_indice))
    {
        pastilla(e, p, m, &mut botones);
    }
    // Abajo: la barra de escuchar o, si no, los mandos de los lados (con la
    // pastilla, o con el raton en la franja de abajo).
    if let Some(v) = e.voz.as_ref().filter(|_| !e.poniendo_marca) {
        for (r, b) in leer_en_voz::pintar_barra(p, m.ancho, m.alto, m.e, v) {
            botones.push((r, Accion::Voz(b)));
        }
    } else if !e.anotando
        && !e.panel
        && !e.poniendo_marca
        && !e.viendo_indice
        // Solo con el raton abajo: al abrir no salen solos (queja del usuario:
        // el «+» solo donde se dibuja; el movil los ensena al tocar).
        && lector::raton_abajo(e.raton.1, m.alto, m.e)
    {
        let (izq, der, tope) = pasos_de_los_lados(e);
        for (r, b) in lector::pintar_mandos_de_los_lados(p, m.ancho, m.alto, m.e, izq, der, tope, e.ajustes.sin_lado) {
            botones.push((r, Accion::Lado(b)));
        }
    }
    // La caja de buscar, arriba en medio (en el sitio de la pastilla).
    if e.nombre.editando.is_none() {
        for (zona, b) in e.hallar.caja.pintar(p, m.ancho, m.e, textos, None) {
            botones.push((zona, Accion::Buscar(b)));
        }
    }
    if e.viendo_indice {
        lista_del_indice(e, p, m, textos, &mut botones);
    }
    if e.panel {
        panel(e, p, m, textos, &mut botones);
    }
    if let Some((texto, _)) = &e.aviso {
        lector::aviso_abajo(p, texto, m.ancho, m.alto, m.e);
    }
    e.botones = botones;
}

/// Una raya fina a la derecha con por donde va la lectura: es lo unico que
/// el «visor limpio» deja ver siempre.
fn barra_de_avance(e: &Estado, p: &Pintor, m: Marco) {
    let s = px(e, m);
    let visto = m.alto / s;
    if e.alto_doc <= visto {
        return;
    }
    let ancho = 3.0 * m.e;
    let x = m.ancho - ancho - 4.0 * m.e;
    let largo = (m.alto * visto / e.alto_doc).max(30.0 * m.e);
    let recorrido = (e.alto_doc - visto).max(1.0);
    let y = (e.y / recorrido).clamp(0.0, 1.0) * (m.alto - largo);
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

/// La pastilla del nombre, con el indice y el engranaje al lado.
fn pastilla(e: &Estado, p: &Pintor, m: Marco, botones: &mut Vec<(RectF, Accion)>) {
    let tam = 15.0 * m.e;
    // El nombre del mensaje del chat, o el del fichero (la pastilla del
    // movil); escribiendolo, lo escrito.
    let nombre = match &e.nombre.editando {
        Some(t) => t.clone(),
        None if e.nombre.visto.is_empty() => pixpin_docs::sin_extension(&e.doc.titulo),
        None => pixpin_docs::sin_extension(&e.nombre.visto),
    };
    let (w, h) = p.medir_texto(&nombre, tam);
    let h = if nombre.is_empty() { p.medir_texto("Ag", tam).1 } else { h };
    let lado = 34.0 * m.e;
    let iconos: Vec<(&pixpin_render::icono::Icono, bool, Accion)> = [
        (!e.indice.is_empty()).then_some((&material::LIST, e.viendo_indice, Accion::Indice)),
        // **Escuchar** (`RecordVoiceOver` de la pastilla del movil).
        Some((&material::RECORD_VOICE_OVER, e.voz.is_some(), Accion::Escuchar)),
        Some((&material::SETTINGS, e.panel, Accion::Engranaje)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let total = w + 28.0 * m.e + lado * iconos.len() as f32;
    let caja = RectF {
        x: (m.ancho - total) / 2.0,
        y: 14.0 * m.e,
        ancho: total,
        alto: h + 14.0 * m.e,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(CRISTAL, 0.94));
    p.texto(&nombre, caja.x + 14.0 * m.e, caja.y + 7.0 * m.e, tam, TEXTO);
    if e.nombre.editando.is_some() {
        // Escribiendo el nombre: un subrayado y el cursor detras, como el
        // campo del movil.
        let x0 = caja.x + 14.0 * m.e;
        p.rellenar(RectF { x: x0, y: caja.y + caja.alto - 6.0 * m.e, ancho: w.max(40.0 * m.e), alto: 1.0 * m.e }, DORADO);
        p.rellenar(RectF { x: x0 + w + 1.0, y: caja.y + 8.0 * m.e, ancho: 1.5 * m.e, alto: h - 2.0 * m.e }, DORADO);
    }
    // El nombre se pulsa para cambiarlo (D5), como en el movil.
    if e.nombre.se_puede() {
        botones.push((RectF { x: caja.x, y: caja.y, ancho: w + 22.0 * m.e, alto: caja.alto }, Accion::Renombrar));
    }
    let x = caja.x + 14.0 * m.e + w + 8.0 * m.e;
    let lado_icono = 19.0 * m.e;
    for (i, (icono, encendido, que)) in iconos.into_iter().enumerate() {
        let zona = RectF {
            x: x - 4.0 * m.e + i as f32 * lado,
            y: caja.y,
            ancho: lado,
            alto: caja.alto,
        };
        p.icono(
            icono,
            RectF {
                x: zona.x + (zona.ancho - lado_icono) / 2.0,
                y: zona.y + (zona.alto - lado_icono) / 2.0,
                ancho: lado_icono,
                alto: lado_icono,
            },
            if encendido { DORADO } else { APAGADO },
        );
        botones.push((zona, que));
    }
}

/// Lo alto de un renglon de la lista del indice.
fn paso_del_indice(m: Marco) -> f32 {
    30.0 * m.e
}

fn caja_del_indice(m: Marco) -> RectF {
    RectF {
        x: 0.0,
        y: 0.0,
        ancho: (380.0 * m.e).min(m.ancho * 0.8),
        alto: m.alto,
    }
}

/// **El indice**, a la izquierda: los capitulos y titulos, con el que se
/// esta leyendo en dorado. Un clic lleva a el.
fn lista_del_indice(
    e: &Estado,
    p: &Pintor,
    m: Marco,
    textos: &Catalogo,
    botones: &mut Vec<(RectF, Accion)>,
) {
    let caja = caja_del_indice(m);
    p.rellenar(caja, CRISTAL);
    p.rellenar(
        RectF {
            x: caja.ancho,
            y: 0.0,
            ancho: 1.0 * m.e,
            alto: m.alto,
        },
        RAYA,
    );
    // La caja entera se apunta primero: un clic en su hueco no es del papel.
    botones.push((caja, Accion::Indice));
    p.texto(
        &textos.t("lector-indice"),
        24.0 * m.e,
        22.0 * m.e,
        13.0 * m.e,
        APAGADO,
    );
    let actual = indice::actual(&e.indice, bloque_arriba(e));
    let paso = paso_del_indice(m);
    let arriba = 56.0 * m.e;
    p.con_recorte(
        RectF {
            x: 0.0,
            y: arriba,
            ancho: caja.ancho,
            alto: m.alto - arriba,
        },
        |p| {
            for (i, entrada) in e.indice.iter().enumerate() {
                let y = arriba + i as f32 * paso - e.corrido_indice;
                if y + paso < arriba || y > m.alto {
                    continue;
                }
                let sangria = (entrada.nivel.saturating_sub(1)) as f32 * 16.0 * m.e;
                let tam = if entrada.nivel <= 1 { 14.0 } else { 13.0 } * m.e;
                let color = if actual == Some(i) { DORADO } else { TEXTO };
                p.texto_linea(
                    &entrada.titulo,
                    24.0 * m.e + sangria,
                    y + 6.0 * m.e,
                    tam,
                    caja.ancho - 40.0 * m.e - sangria,
                    color,
                );
            }
        },
    );
    for i in 0..e.indice.len() {
        let y = arriba + i as f32 * paso - e.corrido_indice;
        if y + paso < arriba || y > m.alto {
            continue;
        }
        botones.push((
            RectF {
                x: 0.0,
                y,
                ancho: caja.ancho,
                alto: paso,
            },
            Accion::IrA(i),
        ));
    }
}

/// El engranaje: una hoja desde abajo con todo a la vista y a un toque,
/// como la del movil.
fn panel(e: &Estado, p: &Pintor, m: Marco, textos: &Catalogo, botones: &mut Vec<(RectF, Accion)>) {
    let filas = filas_del_panel(e, textos);
    let alto_panel = (190.0 + filas.len() as f32 * 28.0) * m.e;
    let caja = RectF {
        x: 0.0,
        y: m.alto - alto_panel,
        ancho: m.ancho,
        alto: alto_panel,
    };
    p.rellenar(caja, CRISTAL);
    p.rellenar(
        RectF {
            x: 0.0,
            y: caja.y,
            ancho: m.ancho,
            alto: 1.0 * m.e,
        },
        RAYA,
    );
    botones.push((caja, Accion::Engranaje));
    let tam = 14.0 * m.e;
    let x = 40.0 * m.e;
    let fijada = e.ajustes.letra_fijada();
    let color_letra = if fijada { APAGADO } else { TEXTO };
    p.texto(&textos.t("visor-tamano"), x, caja.y + 22.0 * m.e, tam, color_letra);
    p.texto(
        &format!("{} %", e.ajustes.tamano),
        x + 200.0 * m.e,
        caja.y + 22.0 * m.e,
        tam,
        APAGADO,
    );
    if fijada {
        p.texto(
            &textos.t("lector-letra-fijada"),
            x + 290.0 * m.e,
            caja.y + 22.0 * m.e,
            12.0 * m.e,
            APAGADO,
        );
    }
    // La barra de puntos: cada punto un tamano, y el elegido, mas gordo.
    let elegido = lectura::punto_del_tamano(e.ajustes.tamano);
    for (i, _) in lectura::TAMANOS.iter().enumerate() {
        let centro = (
            x + 12.0 * m.e + i as f32 * 34.0 * m.e,
            caja.y + 62.0 * m.e,
        );
        let radio = (5.0 + i as f32 * 0.9) * m.e;
        p.circulo(centro, radio, if i == elegido { DORADO } else { RAYA });
        botones.push((
            RectF {
                x: centro.0 - 17.0 * m.e,
                y: centro.1 - 18.0 * m.e,
                ancho: 34.0 * m.e,
                alto: 36.0 * m.e,
            },
            Accion::Tamano(i),
        ));
    }
    // Tipo y grosor: las cuatro letras y los cuatro grosores del movil
    // (`Lectura.LETRAS` y `GROSORES`, K16), la elegida en dorado.
    let y_letra = caja.y + 100.0 * m.e;
    let mut xx = x;
    let letras = ["lector-letra-serif", "lector-letra-sans", "lector-letra-fija", "lector-letra-cursiva"];
    let grosores = ["lector-grosor-fina", "lector-grosor-normal", "lector-grosor-gruesa", "lector-grosor-negra"];
    let pastillas = letras
        .iter()
        .enumerate()
        .map(|(i, k)| (textos.t(k), usize::from(e.ajustes.tipo) == i, Accion::Tipo(i as u8)))
        .chain(
            grosores
                .iter()
                .enumerate()
                .map(|(i, k)| (textos.t(k), usize::from(e.ajustes.grosor) == i, Accion::Grosor(i as u8))),
        )
        .collect::<Vec<_>>();
    for (etiqueta, activa, que) in pastillas {
        let (w, h) = p.medir_texto(&etiqueta, 13.0 * m.e);
        let r = RectF {
            x: xx,
            y: y_letra,
            ancho: w + 24.0 * m.e,
            alto: h + 12.0 * m.e,
        };
        p.rellenar_redondeado(r, r.alto / 2.0, if activa { con_alfa(DORADO, 0.25) } else { RAYA });
        p.texto(
            &etiqueta,
            r.x + 12.0 * m.e,
            r.y + 6.0 * m.e,
            13.0 * m.e,
            if activa { DORADO } else { color_letra },
        );
        botones.push((r, que));
        xx += r.ancho + 8.0 * m.e;
        if matches!(que, Accion::Tipo(t) if usize::from(t) == lectura::TIPOS - 1) {
            xx += 24.0 * m.e;
        }
    }
    for (i, (etiqueta, que)) in filas.into_iter().enumerate() {
        let y = caja.y + (150.0 + i as f32 * 28.0) * m.e;
        p.texto(&etiqueta, x, y, tam, TEXTO);
        botones.push((
            RectF {
                x,
                y: y - 4.0 * m.e,
                ancho: 360.0 * m.e,
                alto: 26.0 * m.e,
            },
            que,
        ));
    }
    p.texto(
        &textos.t("lector-ayuda-docs"),
        x,
        m.alto - 24.0 * m.e,
        12.0 * m.e,
        APAGADO,
    );
}

/// Lo que se puede hacer desde el engranaje, con su texto ya traducido.
fn filas_del_panel(e: &Estado, textos: &Catalogo) -> Vec<(String, Accion)> {
    let mut filas = vec![
        (textos.t("lector-escuchar"), Accion::Escuchar),
        (textos.t("compartir-lector"), Accion::Compartir),
        (textos.t("visor-guardar-pagina"), Accion::GuardarPagina),
        (textos.t("visor-guardar-pdf"), Accion::GuardarPdf),
        (textos.t("visor-abrir-carpeta"), Accion::AbrirCarpeta),
    ];
    if !e.ajustes.marcadores.is_empty() {
        filas.push((textos.t("visor-quitar-marcadores"), Accion::QuitarMarcadores));
    }
    if e.ajustes.letra_fijada() || !e.capa.vacia() {
        filas.push((textos.t("lector-quitar-tinta"), Accion::QuitarTinta));
    }
    filas
}

// ---------------------------------------------------------------------------
// Raton

/// Que hay bajo el raton, segun lo que se apunto al pintar. **Del ultimo al
/// primero**: el que se dibujo encima es el que recibe el clic.
fn que_hay_debajo(e: &Estado) -> Option<Accion> {
    let (x, y) = e.raton;
    e.botones
        .iter()
        .rev()
        .find(|(caja, _)| dentro(caja, x, y))
        .map(|(_, a)| *a)
}

fn riel_de(e: &Estado, m: Marco) -> pixpin_ui::riel_marcas::Riel {
    lector::riel(
        m.ancho,
        m.alto,
        m.escala_por_cien,
        lista_del_riel(e).len(),
        e.poniendo_marca,
    )
}

/// La camara del documento: la misma transformada con la que `pintar` pone
/// la vista (`poner_vista((0,0), s, (-x*s, -y*s))`), para que la tinta
/// traduzca el raton exactamente donde se pinta.
fn camara_del_documento(e: &Estado, m: Marco) -> pixpin_motor2d::camara::Camara {
    pixpin_motor2d::camara::Camara {
        x: e.x,
        y: e.y,
        zoom: px(e, m),
    }
}

/// Un evento para la tinta (`lector_tinta::Tinta`), con la capa del documento.
fn a_la_tinta(e: &mut Estado, ev: &EventoOverlay, m: Marco) -> lector_tinta::Hecho {
    let camara = camara_del_documento(e, m);
    e.tinta.hoja = Some(0);
    e.tinta.evento(ev, &mut e.capa, &camara, m.area, m.escala_por_cien)
}

/// Si el clic es del lector aunque se este anotando: el riel (marcas y
/// dejar de anotar), sus botones, o cerrar lo que hubiera abierto.
fn el_lector_primero(e: &Estado, m: Marco) -> bool {
    !matches!(
        riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1)),
        DestinoRiel::Fuera
    ) || que_hay_debajo(e).is_some()
        || e.panel
        || e.viendo_indice
        || e.poniendo_marca
}

/// El punto del documento bajo el raton (las pruebas de la vista lo usan
/// para comprobar que acercar deja quieto lo que hay bajo el puntero).
#[cfg(test)]
fn en_el_documento(e: &Estado, m: Marco) -> Punto2 {
    let s = px(e, m);
    Punto2::nuevo(e.x + e.raton.0 / s, e.y + e.raton.1 / s)
}

fn mover_raton(e: &mut Estado, m: Marco) {
    let destino = riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1));
    e.encima = destino;
    match e.arrastre {
        Some(Arrastre::Mover {
            desde,
            vista,
            movido,
        }) => {
            let (dx, dy) = (e.raton.0 - desde.0, e.raton.1 - desde.1);
            let movido = movido || dx.abs().max(dy.abs()) > lector::UMBRAL_DE_ARRASTRE;
            if movido {
                let s = px(e, m);
                if !e.ajustes.sin_lado {
                    e.x = vista.0 - dx / s;
                }
                e.y = vista.1 - dy / s;
                e.x_meta = None;
                acotar(e, m);
            }
            e.arrastre = Some(Arrastre::Mover {
                desde,
                vista,
                movido,
            });
        }
        // El trazo lo lleva la tinta, en el bucle.
        Some(Arrastre::Trazo) => {}
        None => {}
    }
}

/// Atiende el clic. Devuelve si empezo un arrastre (para capturar el
/// raton y no perder el soltar si se sale de la ventana).
fn pulsar(e: &mut Estado, textos: &Catalogo, ruta: &Path, ubicacion: &Ubicacion, m: Marco) -> bool {
    e.x_meta = None;
    // Un clic fuera del nombre deja de cambiarlo, sin guardar: se guarda con
    // Intro, como el campo del movil.
    if e.nombre.editando.is_some() && que_hay_debajo(e) != Some(Accion::Renombrar) {
        e.nombre.editando = None;
    }
    let riel = riel_de(e, m);
    match riel.destino(lector::punto(e.raton.0, e.raton.1)) {
        DestinoRiel::PonerMarca => {
            e.poniendo_marca = !e.poniendo_marca;
            return false;
        }
        DestinoRiel::Hojita => {
            alternar_anotar(e, ruta, m);
            return false;
        }
        DestinoRiel::Marca(i) => {
            ir_al_marcador(e, i);
            return false;
        }
        DestinoRiel::Emoji(i) => {
            poner_marcador(e, i, ruta);
            return false;
        }
        DestinoRiel::CerrarTira => {
            e.poniendo_marca = false;
            return false;
        }
        DestinoRiel::Hueco => return false,
        DestinoRiel::Fuera => {}
    }
    if let Some(a) = que_hay_debajo(e) {
        hacer(e, a, textos, ruta, ubicacion, m);
        return false;
    }
    if e.panel || e.viendo_indice || e.poniendo_marca {
        // Un toque en el papel cierra lo que hubiera abierto, como en el
        // movil, sin dibujar ni mover nada.
        e.panel = false;
        e.viendo_indice = false;
        e.poniendo_marca = false;
        return false;
    }
    if e.anotando {
        // Si llega aqui, la tinta no lo quiso (ver el bucle): nada.
        return false;
    }
    e.arrastre = Some(Arrastre::Mover {
        desde: e.raton,
        vista: (e.x, e.y),
        movido: false,
    });
    e.lateral = Some((e.x, ahora_ms()));
    true
}

fn soltar(e: &mut Estado, m: Marco) {
    match e.arrastre.take() {
        Some(Arrastre::Trazo) => {
            e.ultimo_trazo = ahora_ms();
        }
        Some(Arrastre::Mover { movido: false, .. }) => {
            // Un toque en el papel saca (o esconde) la pastilla.
            e.lateral = None;
            e.pastilla_hasta = if e.pastilla_hasta > ahora_ms() {
                0
            } else {
                ahora_ms() + MS_DE_LA_PASTILLA
            };
        }
        Some(Arrastre::Mover { .. }) => fin_de_lateral(e, m),
        None => {}
    }
}

fn pulsar_derecho(e: &mut Estado, ruta: &Path, m: Marco) {
    // Quitar un marcador: clic derecho en su punto del riel (el toque largo
    // del movil, que un raton no tiene).
    if let DestinoRiel::Marca(i) = riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1))
        && let Some(quitado) = lista_del_riel(e).get(i).cloned()
    {
        // El verde es el de la voz: se quita su sitio, no un marcador.
        if quitado.emoji == voz_alta::EMOJI_DE_VOZ {
            e.ajustes.voz = None;
        } else {
            e.ajustes.marcadores.retain(|x| x.id != quitado.id);
        }
        guardar_ajustes(e, ruta);
    }
}

/// Acaba un gesto de lado: si venia hacia la columna, el iman la centra.
fn fin_de_lateral(e: &mut Estado, m: Marco) {
    let Some((antes, _)) = e.lateral.take() else {
        return;
    };
    let s = px(e, m);
    let vista_ancho = m.ancho / s;
    let (izq, der) = limites(e);
    if der - izq <= vista_ancho {
        return;
    }
    let centro = e.columna / 2.0 - vista_ancho / 2.0;
    let margen = vista::margen_de(e.columna);
    if let Some(meta) = vista::iman_del_centro(antes, e.x, centro, margen) {
        e.x_meta = Some(meta);
    }
}

fn rueda(e: &mut Estado, delta: i32, m: Marco) {
    let modificadores = pixpin_shell::entrada::modificadores_pulsados();
    if e.viendo_indice && dentro(&caja_del_indice(m), e.raton.0, e.raton.1) {
        let tope = (e.indice.len() as f32 * paso_del_indice(m) - m.alto * 0.6).max(0.0);
        e.corrido_indice =
            (e.corrido_indice - lector::muescas(delta) * paso_del_indice(m) * 3.0).clamp(0.0, tope);
        return;
    }
    if modificadores.ctrl {
        acercar(e, lector::muescas(delta), m);
    } else if modificadores.shift {
        lado(e, -lector::muescas(delta) * 60.0, m);
    } else {
        e.y += lector::desplazamiento_de_rueda(delta, renglon(e));
        acotar(e, m);
    }
}

/// Corre la vista a lo ancho `cuanto` pixeles.
fn lado(e: &mut Estado, cuanto: f32, m: Marco) {
    // Con el candado de los mandos de abajo, a lo ancho no se mueve.
    if e.ajustes.sin_lado {
        return;
    }
    if e.lateral.is_none() {
        e.lateral = Some((e.x, ahora_ms()));
    }
    if let Some((antes, _)) = e.lateral {
        e.lateral = Some((antes, ahora_ms()));
    }
    e.x_meta = None;
    e.x += cuanto / px(e, m);
    acotar(e, m);
}

/// Acerca (muescas positivas) o aleja **hacia el raton**.
fn acercar(e: &mut Estado, muescas: f32, m: Marco) {
    let antes = px(e, m);
    let (izq, der) = limites(e);
    let minimo = vista::zoom_minimo(der - izq, m.ancho, m.e);
    e.zoom = vista::zoom_con_rueda(e.zoom, muescas, minimo, vista::ZOOM_MAXIMO);
    let despues = px(e, m);
    e.x = vista::con_foco(e.x, e.raton.0, antes, despues);
    e.y = vista::con_foco(e.y, e.raton.1, antes, despues);
    e.x_meta = None;
    acotar(e, m);
}

// ---------------------------------------------------------------------------
// Teclado

/// Atiende una tecla. Devuelve `false` si hay que cerrar el lector.
#[allow(clippy::too_many_arguments)] // una tecla depende de sus modificadores y del documento
fn tecla(
    e: &mut Estado,
    vk: u32,
    shift: bool,
    ctrl: bool,
    _alt: bool,
    textos: &Catalogo,
    ruta: &Path,
    m: Marco,
) -> bool {
    let linea = renglon(e);
    let pantalla = m.alto / px(e, m) * 0.9;
    // Anotando, una letra que elige una herramienta es de la herramienta
    // (la tinta la atiende por su caracter): la M de la mano no pone un
    // marcador ni la G de la escala grafica abre los ajustes.
    if e.anotando && !ctrl && lector_tinta::letra_de_herramienta(vk) {
        return true;
    }
    match (vk, ctrl) {
        (VK_ESCAPE, _) => {
            if e.poniendo_marca || e.panel || e.viendo_indice {
                e.poniendo_marca = false;
                e.panel = false;
                e.viendo_indice = false;
            } else if e.voz.is_some() {
                // La primera Esc cierra la barra de escuchar; la segunda, el lector.
                e.voz = None;
            } else if e.anotando {
                alternar_anotar(e, ruta, m);
            } else {
                return false;
            }
        }
        (VK_DOWN, false) => e.y += linea,
        (VK_UP, false) => e.y -= linea,
        (VK_NEXT, _) => e.y += pantalla,
        (VK_PRIOR, _) => e.y -= pantalla,
        (VK_ESPACIO, false) => e.y += if shift { -pantalla } else { pantalla },
        (VK_HOME, _) => e.y = 0.0,
        (VK_END, _) => e.y = e.alto_doc,
        (VK_LEFT, false) => lado(e, -60.0 * m.e, m),
        (VK_RIGHT, false) => lado(e, 60.0 * m.e, m),
        (VK_0, true) => {
            e.zoom = 1.0;
            e.x_meta = None;
        }
        (VK_MAS | VK_MAS_NUM, true) => {
            e.raton = (m.ancho / 2.0, m.alto / 2.0);
            acercar(e, 1.0, m);
        }
        (VK_MENOS | VK_MENOS_NUM, true) => {
            e.raton = (m.ancho / 2.0, m.alto / 2.0);
            acercar(e, -1.0, m);
        }
        (VK_MAS | VK_MAS_NUM, false) => cambiar_letra(e, textos, ruta, |a| {
            a.tamano = lectura::tamano_vecino(a.tamano, true)
        }),
        (VK_MENOS | VK_MENOS_NUM, false) => cambiar_letra(e, textos, ruta, |a| {
            a.tamano = lectura::tamano_vecino(a.tamano, false)
        }),
        (VK_T, false) if !e.anotando => {
            cambiar_letra(e, textos, ruta, |a| a.tipo = (a.tipo + 1) % lectura::TIPOS as u8)
        }
        (VK_B, false) if !e.anotando => cambiar_letra(e, textos, ruta, |a| {
            a.grosor = (a.grosor + 1) % lectura::GROSORES as u8
        }),
        (VK_Z, true) => {
            if shift {
                e.capa.rehacer();
            } else {
                e.capa.deshacer();
            }
            e.ultimo_trazo = ahora_ms();
        }
        (VK_Y, true) => {
            e.capa.rehacer();
            e.ultimo_trazo = ahora_ms();
        }
        (VK_S, true) => guardar_capa(e, ruta),
        (VK_M, _) => {
            e.poniendo_marca = !e.poniendo_marca;
            e.panel = false;
        }
        (VK_A, false) => alternar_anotar(e, ruta, m),
        (VK_L, false) => escuchar(e, textos, ruta, m),
        (VK_G, false) => {
            e.panel = !e.panel;
            e.viendo_indice = false;
        }
        (VK_I, false) if !e.indice.is_empty() => {
            e.viendo_indice = !e.viendo_indice;
            e.panel = false;
            centrar_indice(e, m);
        }
        (v, true) if (0x31..=0x39).contains(&v) => ir_al_marcador(e, (v - 0x31) as usize),
        _ => e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA,
    }
    acotar(e, m);
    true
}

fn caracter(e: &mut Estado, c: char, ruta: &Path) {
    if e.poniendo_marca {
        if let Some(i) = lector::emoji_de_cifra(c) {
            poner_marcador(e, i, ruta);
        }
    }
    // Anotando, las cifras son de la tinta (el grosor, como en el lienzo) y
    // ya las atendio ella en el bucle.
}

/// Deja la entrada de ahora a la vista al abrir el indice.
fn centrar_indice(e: &mut Estado, m: Marco) {
    if let Some(i) = indice::actual(&e.indice, bloque_arriba(e)) {
        let paso = paso_del_indice(m);
        e.corrido_indice = (i as f32 * paso - m.alto / 3.0).max(0.0);
    } else {
        e.corrido_indice = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Hacer

/// Cambia algo de la letra, **si se puede**: con tinta encima la letra esta
/// fijada, porque el texto se partiria en otras lineas y lo anotado quedaria
/// sobre otras palabras. Se dice por que no, en vez de no hacer nada.
fn cambiar_letra(
    e: &mut Estado,
    textos: &Catalogo,
    ruta: &Path,
    cambio: impl FnOnce(&mut lectura::Ajustes),
) {
    if e.ajustes.letra_fijada() {
        e.aviso = Some((textos.t("lector-letra-fijada"), ahora_ms() + 3000));
        return;
    }
    cambio(&mut e.ajustes);
    e.ajustes.tamano = lectura::tamano_valido(e.ajustes.tamano);
    // Se vuelve a medir en el proximo fotograma, al mismo punto.
    e.medido_con = None;
    guardar_ajustes(e, ruta);
    e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
}

fn alternar_anotar(e: &mut Estado, ruta: &Path, m: Marco) {
    if e.anotando {
        e.anotando = false;
        // Lo que se escribia se queda escrito, y lo elegido se suelta.
        if e.tinta.gesto.esta_escribiendo() {
            e.tinta.gesto.cerrar_texto(&mut e.capa.escena);
            e.capa.sucia = true;
        }
        e.tinta.gesto.seleccion.limpiar();
        e.tinta.gesto.lazo = None;
        guardar_capa(e, ruta);
        return;
    }
    // **La primera vez se fija la columna** (y con ella la letra): es lo que
    // hace que lo anotado siga encima de su palabra la proxima vez, aunque
    // la ventana sea otra.
    if !e.ajustes.letra_fijada() {
        e.ajustes.columna = columna_que_cabe(m).round() as u32;
        guardar_ajustes(e, ruta);
    }
    e.anotando = true;
    e.poniendo_marca = false;
    e.panel = false;
    e.viendo_indice = false;
}

fn poner_marcador(e: &mut Estado, i: usize, ruta: &Path) {
    if i == lector::VERDE_EN_LA_TIRA {
        poner_el_verde_aqui(e, ruta);
        return;
    }
    let emoji = EMOJIS.get(i).copied().unwrap_or(EMOJIS[0]);
    e.ajustes.marcadores =
        lectura::con_marcador(&e.ajustes.marcadores, fraccion(e), emoji, ahora_ms());
    e.poniendo_marca = false;
    guardar_ajustes(e, ruta);
}

fn ir_al_marcador(e: &mut Estado, i: usize) {
    if let Some(m) = lista_del_riel(e).get(i) {
        e.y = e.alto_doc * m.fraccion;
    }
}

/// Los puntos del riel: los marcadores y el verde de la voz en su sitio
/// (`Lectura.conMarcaDeVoz`).
fn lista_del_riel(e: &Estado) -> Vec<lectura::Marcador> {
    voz_alta::con_marca_de_voz(&e.ajustes.marcadores, e.ajustes.voz.map(|v| v.1))
}

// ---------------------------------------------------------------------------
// Escuchar (la voz de Windows) y el espacio de los lados

/// Los colocados que se leen, en orden: los que tienen texto (ni el hueco
/// de una imagen ni una raya). Cada uno es un «parrafo» de la voz, como cada
/// bloque de texto sin otro dentro en el movil (`VozAlta.PREPARAR`).
pub(crate) fn para_leer(colocados: &[Colocado]) -> Vec<usize> {
    colocados
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.recuadro && !matches!(c.clase, Clase::Regla | Clase::Capitulo) && !c.texto.trim().is_empty())
        .map(|(i, _)| i)
        .collect()
}

fn colocado_que_suena(e: &Estado) -> Option<&Colocado> {
    let v = e.voz.as_ref()?;
    let i = *para_leer(&e.colocados).get(v.actual)?;
    e.colocados.get(i)
}

/// Donde empieza cada parrafo de la voz, de 0 a 1.
fn fracciones_de(e: &Estado, idx: &[usize]) -> Vec<f32> {
    let alto = e.alto_doc.max(1.0);
    idx.iter().map(|i| (e.colocados[*i].y / alto).clamp(0.0, 1.0)).collect()
}

/// El primer parrafo que asoma arriba de la ventana.
fn parrafo_arriba(e: &Estado, idx: &[usize]) -> usize {
    idx.iter()
        .position(|i| {
            let c = &e.colocados[*i];
            c.y + c.alto > e.y + 4.0
        })
        .unwrap_or(0)
}

/// **Escuchar** (el boton de la pastilla y la L): la primera vez se arranca
/// la voz de Windows con el idioma del texto y se empieza **por el marcador
/// verde**, donde se dejo de escuchar; si no hay, por lo que asoma arriba.
/// Con la barra ya abierta, play/pausa.
fn escuchar(e: &mut Estado, textos: &Catalogo, ruta: &Path, m: Marco) {
    if let Some(v) = e.voz.as_mut() {
        v.alternar();
        return;
    }
    let idx = para_leer(&e.colocados);
    if idx.is_empty() {
        e.aviso = Some((textos.t("lector-sin-texto"), ahora_ms() + 3000));
        return;
    }
    let fracciones = fracciones_de(e, &idx);
    let desde = e
        .ajustes
        .voz
        .and_then(|marca| voz_alta::parrafo_de_la_marca(&fracciones, marca))
        .unwrap_or_else(|| parrafo_arriba(e, &idx));
    let parrafos: Vec<String> = idx.iter().map(|i| voz_alta::juntar_blancos(&e.colocados[*i].texto)).collect();
    let muestra: String = parrafos.iter().skip(desde).take(40).cloned().collect::<Vec<_>>().join(" ").chars().take(4000).collect();
    let de_la_app = leer_en_voz::idioma_de_la_app(textos);
    let idioma = voz_alta::idioma_para_leer(&muestra, "", &de_la_app);
    let Some(mut v) = LeerEnVoz::arrancar(&idioma, &de_la_app, parrafos, 1.0) else {
        e.aviso = Some((leer_en_voz::sin_voz(textos, &idioma), ahora_ms() + 6000));
        return;
    };
    v.leer(desde);
    e.voz = Some(v);
    e.poniendo_marca = false;
    e.panel = false;
    al_sonar(e, desde, ruta, m);
}

/// **Suena otro parrafo**: el verde se mueve a el (en cada parrafo, no solo
/// al parar, como `apuntarElVerde`) y, si el de antes se estaba viendo, la
/// vista lo sigue (`VozAlta.resaltar` con `seguir`; anotando no se mueve).
fn al_sonar(e: &mut Estado, i: usize, ruta: &Path, m: Marco) {
    let idx = para_leer(&e.colocados);
    let Some(&k) = idx.get(i) else {
        return;
    };
    let alto = e.alto_doc.max(1.0);
    let c = &e.colocados[k];
    let (y, abajo) = (c.y, c.y + c.alto);
    let visto = m.alto / px(e, m);
    let se_veia = |a: f32, b: f32| b > e.y && a < e.y + visto;
    let antes_se_veia = i
        .checked_sub(1)
        .and_then(|j| idx.get(j))
        .map(|j| {
            let a = &e.colocados[*j];
            se_veia(a.y, a.y + a.alto)
        })
        .unwrap_or(true);
    if !e.anotando && antes_se_veia && (y < e.y + visto * 0.08 || abajo > e.y + visto * 0.85) {
        e.y = (y - visto * 0.25).max(0.0);
        acotar(e, m);
    }
    e.ajustes.voz = Some((i, (y / alto).clamp(0.0, 1.0)));
    guardar_ajustes(e, ruta);
}

/// **El marcador verde, aqui** («poder mover el bookmark verde donde quiero
/// que empiece a leer»): al primer parrafo que asoma arriba. Escuchando, la
/// voz salta ahi ya.
fn poner_el_verde_aqui(e: &mut Estado, ruta: &Path) {
    e.poniendo_marca = false;
    let idx = para_leer(&e.colocados);
    if idx.is_empty() {
        return;
    }
    let p = parrafo_arriba(e, &idx);
    let f = (e.colocados[idx[p]].y / e.alto_doc.max(1.0)).clamp(0.0, 1.0);
    e.ajustes.voz = Some((p, f));
    guardar_ajustes(e, ruta);
    if let Some(v) = e.voz.as_mut() {
        if v.leyendo {
            v.leer(p);
        } else {
            let actual = v.actual as isize;
            v.saltar(p as isize - actual);
        }
    }
}

/// Un boton de la barra de escuchar.
fn boton_de_voz(e: &mut Estado, b: leer_en_voz::BotonVoz, ruta: &Path, m: Marco) {
    use leer_en_voz::BotonVoz;
    let Some(v) = e.voz.as_mut() else {
        return;
    };
    match b {
        BotonVoz::Alternar => v.alternar(),
        BotonVoz::Anterior | BotonVoz::Siguiente => {
            let i = v.saltar(if b == BotonVoz::Anterior { -1 } else { 1 });
            al_sonar(e, i, ruta, m);
        }
        BotonVoz::Velocidad => {
            v.otra_velocidad();
        }
        BotonVoz::Cerrar => e.voz = None,
    }
}

/// Los pasos de espacio puestos a cada lado y cuantos caben (sin columna
/// fijada, ninguno y dos, como el movil).
fn pasos_de_los_lados(e: &Estado) -> (u32, u32, u32) {
    if !e.ajustes.letra_fijada() {
        return (0, 0, 2);
    }
    let c = e.ajustes.columna;
    let m = c * 2 / 3;
    let (i, d) = e.ajustes.lados.unwrap_or((m, m));
    let (pi, tope) = vista::pasos_de_espacio(i, c);
    let (pd, _) = vista::pasos_de_espacio(d, c);
    (pi, pd, tope)
}

/// **Espacio en blanco a un lado** (`anadirEspacio` del movil): un paso mas
/// o uno menos. Si la columna aun no esta fijada, se fija aqui, sin espacio
/// a ningun lado salvo el pedido. En el PC la tinta cuenta desde el borde
/// de la columna, asi que el texto y lo anotado no se mueven; la del
/// mensaje del chat se escribe antes con su marco (`.hoja`) en las unidades
/// de ahora, para que el `izq` nuevo de la maqueta no la corra.
fn anadir_espacio(e: &mut Estado, izquierda: bool, mas: bool, textos: &Catalogo, ruta: &Path) {
    if !e.ajustes.letra_fijada() {
        e.ajustes.columna = e.columna.round().max(1.0) as u32;
        e.ajustes.lados = Some((0, 0));
    }
    let c = e.ajustes.columna;
    let m = c * 2 / 3;
    let (i, d) = e.ajustes.lados.unwrap_or((m, m));
    let paso = i64::from(vista::paso_de_espacio(c)) * if mas { 1 } else { -1 };
    let antes = if izquierda { i } else { d };
    let ahora = vista::espacio_valido(i64::from(antes) + paso, c);
    if ahora == antes {
        let clave = if mas { "lector-espacio-tope" } else { "lector-espacio-nada" };
        e.aviso = Some((textos.t(clave), ahora_ms() + 2500));
        return;
    }
    if !e.capa.vacia() {
        e.capa.sucia = true;
        guardar_capa(e, ruta);
    }
    e.ajustes.lados = Some(if izquierda { (ahora, d) } else { (i, ahora) });
    guardar_ajustes(e, ruta);
    e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
}

fn boton_de_lado(e: &mut Estado, b: lector::BotonLado, textos: &Catalogo, ruta: &Path) {
    match b {
        lector::BotonLado::Izquierda(mas) => anadir_espacio(e, true, mas, textos, ruta),
        lector::BotonLado::Derecha(mas) => anadir_espacio(e, false, mas, textos, ruta),
        lector::BotonLado::Candado => {
            e.ajustes.sin_lado = !e.ajustes.sin_lado;
            let clave = if e.ajustes.sin_lado { "lector-sin-lado" } else { "lector-con-lado" };
            e.aviso = Some((textos.t(clave), ahora_ms() + 2500));
            e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
            guardar_ajustes(e, ruta);
        }
    }
}

fn hacer(e: &mut Estado, a: Accion, textos: &Catalogo, ruta: &Path, ubicacion: &Ubicacion, m: Marco) {
    match a {
        Accion::Renombrar => e.nombre.empezar(),
        Accion::Escuchar => escuchar(e, textos, ruta, m),
        Accion::Voz(b) => boton_de_voz(e, b, ruta, m),
        Accion::Lado(b) => boton_de_lado(e, b, textos, ruta),
        Accion::Buscar(b) => {
            let h = e.hallar.caja.boton(b);
            hecho_de_buscar(e, h);
        }
        Accion::Indice => {
            // La caja entera del indice tambien es esta accion: pulsar en su
            // hueco no la cierra (se cierra con el boton, Esc o fuera).
            if !dentro(&caja_del_indice(m), e.raton.0, e.raton.1) || !e.viendo_indice {
                e.viendo_indice = !e.viendo_indice;
                e.panel = false;
                centrar_indice(e, m);
            }
        }
        Accion::Engranaje => {
            // Igual: dentro de la hoja no se cierra.
            let en_la_hoja = e.panel && e.raton.1 > m.alto * 0.5;
            if !en_la_hoja {
                e.panel = !e.panel;
                e.viendo_indice = false;
            }
        }
        Accion::IrA(i) => {
            if let Some(entrada) = e.indice.get(i) {
                // Un renglon por encima: el titulo no queda pegado al borde.
                e.y = (y_del_bloque(e, entrada.bloque) - renglon(e)).max(0.0);
            }
        }
        Accion::QuitarMarcadores => {
            e.ajustes.marcadores.clear();
            guardar_ajustes(e, ruta);
        }
        Accion::QuitarTinta => {
            e.capa.vaciar();
            guardar_capa(e, ruta);
            // Sin tinta, la letra vuelve a ser libre.
            e.ajustes.columna = 0;
            e.anotando = false;
            e.medido_con = None;
            guardar_ajustes(e, ruta);
            e.aviso = Some((textos.t("lector-tinta-quitada"), ahora_ms() + 4000));
        }
        Accion::Tamano(i) => cambiar_letra(e, textos, ruta, |a| {
            a.tamano = lectura::TAMANOS[i.min(lectura::TAMANOS.len() - 1)]
        }),
        Accion::Tipo(t) => cambiar_letra(e, textos, ruta, |a| a.tipo = t),
        Accion::Grosor(g) => cambiar_letra(e, textos, ruta, |a| a.grosor = g),
        // **La pagina web con lo anotado y los marcadores**, en la que se
        // sigue anotando: la misma que sale al compartir, hecha con la
        // disposicion que hay en pantalla (no hace falta volver a medir).
        Accion::GuardarPagina => {
            let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
            let medidas = medidas_de_ahora(e);
            let hoja = documento_web::hoja_de_texto(
                &titulo,
                &e.doc,
                &e.ajustes,
                &e.capa.escena,
                e.columna,
                medidas.as_ref(),
                &documento_web::imagen_para_la_web,
            );
            let hecho = documento_web::pagina(&[hoja], &titulo)
                .ok_or_else(|| std::io::Error::other("sin pagina"))
                .and_then(|pagina| guardar(ubicacion, ruta, "html", pagina.as_bytes()));
            dejar_medidas(e, ruta);
            avisar(e, textos, hecho);
        }
        Accion::GuardarPdf => {
            let bytes = pixpin_docs::pdf::de_documento(&e.doc);
            avisar(e, textos, guardar(ubicacion, ruta, "pdf", &bytes));
        }
        // Lo anotado, a disco antes: la hoja lo lee de alli, en su hilo.
        Accion::Compartir => {
            guardar_capa(e, ruta);
            guardar_ajustes(e, ruta);
            dejar_medidas(e, ruta);
            e.panel = false;
            crate::compartir::ventana::abrir(
                crate::compartir::idioma_de(ubicacion),
                ubicacion.clone(),
                crate::compartir::Cosa::Documento(ruta.to_path_buf()),
            );
        }
        Accion::AbrirCarpeta => {
            let _ = pixpin_shell::abrir_ubicacion(&carpeta_de_salida(ubicacion));
        }
    }
}

fn guardar_ajustes(e: &Estado, ruta: &Path) {
    if let Err(err) = crate::anotado_del_adjunto::escribir(ruta, &e.ajustes) {
        // Un documento en un sitio de solo lectura no tiene por que
        // estropear la lectura: se apunta y ya.
        tracing::info!(?err, "no se pudo guardar la lectura junto al documento");
    }
}

/// Las medidas de lo que hay colocado en pantalla, para la pagina web
/// anotada. `None` si todavia no se midio nada.
fn medidas_de_ahora(e: &Estado) -> Option<documento_web::Medidas> {
    (!e.colocados.is_empty())
        .then(|| documento_web::medidas_de(&e.doc, &e.colocados, e.alto_doc, tamano_base(&e.ajustes)))
}

/// **Deja las medidas junto al documento** (`guardarMedidas` del movil):
/// asi la pagina web anotada sale con cada cosa en su parrafo aunque se
/// exporte sin el lector abierto y sin poder medir.
fn dejar_medidas(e: &Estado, ruta: &Path) {
    let Some(m) = medidas_de_ahora(e) else {
        return;
    };
    if let Err(err) = documento_web::guardar_medidas(ruta, &e.ajustes, e.columna, &m, !e.capa.vacia()) {
        tracing::info!(?err, "no se pudieron dejar las medidas junto al documento");
    }
}

fn guardar_capa(e: &mut Estado, ruta: &Path) {
    if let Err(err) = crate::anotado_del_adjunto::guardar_capa(ruta, &mut e.capa) {
        tracing::warn!(?err, "no se pudo guardar lo anotado junto al documento");
    }
}

/// Donde van las cosas que el visor fabrica. No se escribe al lado del
/// documento: el documento puede estar en una carpeta que no es nuestra (o
/// de solo lectura), y una copia inesperada al lado de un archivo ajeno es
/// justo lo que nadie espera.
fn carpeta_de_salida(ubicacion: &Ubicacion) -> PathBuf {
    ubicacion.raiz().join("exportado")
}

fn guardar(ubicacion: &Ubicacion, ruta: &Path, extension: &str, datos: &[u8]) -> std::io::Result<PathBuf> {
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
    e.aviso = Some((texto, ahora_ms() + 3500));
}

// ---------------------------------------------------------------------------
// Buscar dentro (D9) y cambiar el nombre (D5)

/// Lo de buscar: la caja (`buscador`), y donde cae cada coincidencia, en
/// unidades del documento. Las cajas se piden a DirectWrite la primera vez
/// que una coincidencia se ve, y se guardan hasta que cambie la busqueda o
/// la medida: un libro con mil «de» no pregunta mil veces por fotograma.
#[derive(Default)]
struct Hallar {
    caja: crate::buscador::Buscador,
    cajas: std::collections::HashMap<usize, Vec<RectF>>,
    /// En el siguiente fotograma, llevar la vista a la coincidencia actual.
    saltar: bool,
}

/// Si lo que se teclea es de una caja (buscar o el nombre) y no atajos.
fn escribiendo(e: &Estado) -> bool {
    e.hallar.caja.abierto || e.nombre.editando.is_some()
}

/// Las teclas de las dos cajas. Devuelve si era suya y, si al cambiar el
/// nombre se movio el fichero, su ruta nueva.
fn tecla_de_las_cajas(
    e: &mut Estado,
    vk: u32,
    shift: bool,
    ctrl: bool,
    textos: &Catalogo,
    ruta: &Path,
    ubicacion: &Ubicacion,
) -> (bool, Option<PathBuf>) {
    use crate::renombrar_doc::Tecla;
    match e.nombre.tecla(vk, ctrl) {
        Tecla::Consumida => return (true, None),
        Tecla::Confirmar => {
            // Lo pendiente se escribe antes al nombre de ahora: si el
            // fichero se mueve, se lleva ya su tinta y su sitio.
            guardar_ajustes(e, ruta);
            guardar_capa(e, ruta);
            let hecho = e.nombre.confirmar(ubicacion.raiz(), ruta, textos);
            if let Some(t) = hecho.aviso {
                e.aviso = Some((t, ahora_ms() + 3500));
            }
            e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
            return (true, hecho.ruta_nueva);
        }
        Tecla::NoEsMia if e.nombre.editando.is_some() => return (false, None),
        Tecla::NoEsMia => {}
    }
    let h = e.hallar.caja.tecla(vk, shift, ctrl);
    if h == crate::buscador::Hecho::NoEsMia {
        return (false, None);
    }
    hecho_de_buscar(e, h);
    (true, None)
}

/// Un caracter de las dos cajas. `false` si no era de ninguna.
fn caracter_de_las_cajas(e: &mut Estado, c: char) -> bool {
    if e.nombre.caracter(c) {
        return true;
    }
    match e.hallar.caja.caracter(c) {
        Some(h) => {
            hecho_de_buscar(e, h);
            true
        }
        None => false,
    }
}

fn hecho_de_buscar(e: &mut Estado, h: crate::buscador::Hecho) {
    use crate::buscador::Hecho;
    match h {
        // Al abrir sigue lo que se busco la ultima vez, como en el navegador.
        Hecho::Abierta | Hecho::Escrito => rebuscar(e),
        Hecho::Siguiente | Hecho::Anterior => {
            e.hallar.caja.abierto = true;
            if e.hallar.caja.busqueda.coincidencias.is_empty() {
                rebuscar(e);
            } else if h == Hecho::Siguiente {
                e.hallar.caja.busqueda.siguiente();
            } else {
                e.hallar.caja.busqueda.anterior();
            }
            e.hallar.saltar = true;
        }
        Hecho::Cerrada | Hecho::Nada | Hecho::NoEsMia => {}
    }
}

/// Vuelve a buscar lo escrito en lo que se pinta (los bloques colocados,
/// con la vineta de las listas y todo), empezando por lo que se esta leyendo.
fn rebuscar(e: &mut Estado) {
    let desde = e.colocados.partition_point(|c| c.y + c.alto < e.y);
    e.hallar
        .caja
        .busqueda
        .rehacer(e.colocados.iter().map(|c| c.texto.as_str()), desde);
    e.hallar.cajas.clear();
    e.hallar.saltar = e.hallar.caja.busqueda.la_actual().is_some();
}

/// Las cajas de la coincidencia `i` en unidades del documento, pidiendolas
/// si aun no se tenian.
fn cajas_de_coincidencia(e: &mut Estado, p: &Pintor, i: usize) -> Vec<RectF> {
    if let Some(c) = e.hallar.cajas.get(&i) {
        return c.clone();
    }
    let Some(k) = e.hallar.caja.busqueda.coincidencias.get(i).copied() else {
        return Vec::new();
    };
    let Some(col) = e.colocados.get(k.texto) else {
        return Vec::new();
    };
    let inicio = pixpin_docs::buscar::a_utf16(&col.texto, k.inicio);
    let fin = pixpin_docs::buscar::a_utf16(&col.texto, k.inicio + k.largo);
    let cajas: Vec<RectF> = p
        .cajas_de_lectura(
            &col.texto,
            col.tam,
            col.ancho,
            &col.tramos,
            &col.letra.para_pintar(),
            &col.renglones,
            inicio as u32,
            (fin - inicio) as u32,
        )
        .into_iter()
        .map(|r| RectF {
            x: r.x + col.sangria,
            y: r.y + col.y,
            ..r
        })
        .collect();
    e.hallar.cajas.insert(i, cajas.clone());
    cajas
}

/// Marca lo encontrado que se ve; la actual, mas fuerte. Con la caja
/// cerrada no se marca nada, como en el navegador.
fn marcas_de_busqueda(e: &mut Estado, p: &Pintor, vista: (f32, f32, f32, f32)) {
    if !e.hallar.caja.abierto {
        return;
    }
    let actual = e.hallar.caja.busqueda.actual;
    // Las coincidencias van en orden de lectura, y los bloques tambien: se
    // salta de golpe a la primera que puede verse.
    let primera_visible = e.colocados.partition_point(|c| c.y + c.alto < vista.1);
    let desde = e
        .hallar
        .caja
        .busqueda
        .coincidencias
        .partition_point(|k| k.texto < primera_visible);
    for i in desde..e.hallar.caja.busqueda.coincidencias.len() {
        let k = e.hallar.caja.busqueda.coincidencias[i];
        if e.colocados.get(k.texto).is_none_or(|c| c.y > vista.3) {
            break;
        }
        let color = if Some(i) == actual {
            crate::buscador::MARCA_ACTUAL
        } else {
            crate::buscador::MARCA
        };
        for r in cajas_de_coincidencia(e, p, i) {
            p.rellenar_redondeado(r, 2.0, color);
        }
    }
}

/// Lleva la vista a la coincidencia actual si no se ve: a un tercio de la
/// ventana, que se lea tambien lo de antes.
fn saltar_a_la_actual(e: &mut Estado, p: &Pintor, m: Marco) {
    e.hallar.saltar = false;
    let Some(i) = e.hallar.caja.busqueda.actual else {
        return;
    };
    let cajas = cajas_de_coincidencia(e, p, i);
    let Some(r) = cajas.first().copied() else {
        return;
    };
    let s = px(e, m);
    let (ancho, alto) = (m.ancho / s, m.alto / s);
    // Arriba queda la caja de buscar: lo que cae debajo de ella no se ve.
    let tapado = 70.0 * m.e / s;
    if r.y < e.y + tapado || r.y + r.alto > e.y + alto {
        e.y = r.y - alto / 3.0;
    }
    if r.x < e.x || r.x + r.ancho > e.x + ancho {
        e.x = r.x - ancho / 3.0;
        e.x_meta = None;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::documento::{Bloque, Trozo};

    fn marco() -> Marco {
        Marco {
            ancho: 1600.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect { x: 0, y: 0, ancho: 1600, alto: 900 },
        }
    }

    pub(super) fn estado_de_prueba() -> Estado {
        Estado {
            doc: Documento::default(),
            indice: Vec::new(),
            ajustes: lectura::Ajustes::default(),
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            alto_doc: 10_000.0,
            columna: 800.0,
            colocados: Vec::new(),
            medido_con: None,
            pastilla_hasta: 0,
            panel: false,
            viendo_indice: false,
            corrido_indice: 0.0,
            poniendo_marca: false,
            encima: DestinoRiel::Fuera,
            aviso: None,
            botones: Vec::new(),
            raton: (0.0, 0.0),
            anotando: false,
            capa: Capa::default(),
            tinta: Tinta::nueva(),
            ultimo_trazo: 0,
            arrastre: None,
            lateral: None,
            x_meta: None,
            hallar: Hallar::default(),
            nombre: Default::default(),
            hoja: Hoja::Word,
            tinta_de_antes: false,
            guardar_ya: false,
            voz: None,
        }
    }

    fn colocado_para_leer(texto: &str, clase: Clase, y: f32, recuadro: bool) -> Colocado {
        Colocado {
            texto: texto.into(),
            tramos: Vec::new(),
            clase,
            tam: 16.0,
            sangria: 0.0,
            ancho: 800.0,
            y,
            alto: 40.0,
            color: TEXTO,
            bloque: Some(0),
            caja: None,
            letra: Letra::del_cuerpo(Hoja::Word, &lectura::Ajustes::default()),
            recuadro,
            renglones: Vec::new(),
        }
    }

    #[test]
    fn la_voz_lee_lo_que_tiene_texto_y_no_imagenes_ni_rayas() {
        let c = vec![
            colocado_para_leer("Titulo", Clase::Titulo(1), 0.0, false),
            colocado_para_leer("", Clase::Parrafo, 50.0, false),
            colocado_para_leer("[imagen]", Clase::Parrafo, 100.0, true),
            colocado_para_leer("", Clase::Regla, 150.0, false),
            colocado_para_leer("Un parrafo.", Clase::Parrafo, 200.0, false),
        ];
        assert_eq!(para_leer(&c), vec![0, 4]);
    }

    #[test]
    fn se_empieza_por_el_verde_y_se_mueve_con_lo_que_suena() {
        let dir = std::env::temp_dir().join("pixpin-visor-voz");
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("libro.md");
        std::fs::write(&ruta, "x").unwrap();
        let mut e = estado_de_prueba();
        e.alto_doc = 1000.0;
        e.colocados = (0..5).map(|i| colocado_para_leer("Algo.", Clase::Parrafo, i as f32 * 200.0, false)).collect();
        let idx = para_leer(&e.colocados);
        let f = fracciones_de(&e, &idx);
        assert_eq!(voz_alta::parrafo_de_la_marca(&f, (3, 0.6)), Some(3));
        // Lo que asoma arriba con la vista a media pagina.
        e.y = 390.0;
        assert_eq!(parrafo_arriba(&e, &idx), 2);
        al_sonar(&mut e, 4, &ruta, marco());
        assert_eq!(e.ajustes.voz, Some((4, 0.8)));
        assert_eq!(lectura::leer(&ruta).voz, Some((4, 0.8)), "el verde queda junto al documento");
        // El verde sale en el riel y quitarlo no toca los marcadores.
        e.ajustes.marcadores = lectura::con_marcador(&[], 0.1, "⭐", 1);
        let lista = lista_del_riel(&e);
        assert_eq!(lista.iter().map(|m| m.emoji.as_str()).collect::<Vec<_>>(), vec!["⭐", voz_alta::EMOJI_DE_VOZ]);
    }

    #[test]
    fn los_mandos_abren_espacio_a_tercios_y_el_candado_bloquea_el_lado() {
        let dir = std::env::temp_dir().join("pixpin-visor-lados");
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("tesis.md");
        std::fs::write(&ruta, "x").unwrap();
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut e = estado_de_prueba();
        e.columna = 600.0;
        assert_eq!(limites(&e).0, 0.0, "sin columna fijada no hay margenes");
        assert_eq!(pasos_de_los_lados(&e), (0, 0, 2));
        // Sin columna fijada se fija aqui, con solo el espacio pedido.
        anadir_espacio(&mut e, false, true, &textos, &ruta);
        assert_eq!(e.ajustes.columna, 600);
        assert_eq!(e.ajustes.lados, Some((0, 200)));
        assert_eq!(limites(&e), (0.0, 800.0));
        anadir_espacio(&mut e, false, true, &textos, &ruta);
        anadir_espacio(&mut e, false, true, &textos, &ruta);
        assert_eq!(e.ajustes.lados, Some((0, 400)), "como mucho dos pasos");
        assert!(e.aviso.is_some());
        anadir_espacio(&mut e, true, true, &textos, &ruta);
        assert_eq!(pasos_de_los_lados(&e), (1, 2, 2));
        assert_eq!(limites(&e), (-200.0, 1000.0));
        assert_eq!(lectura::leer(&ruta).lados, Some((200, 400)), "se guarda");
        // El candado: a lo ancho no se mueve.
        boton_de_lado(&mut e, lector::BotonLado::Candado, &textos, &ruta);
        let x = e.x;
        lado(&mut e, 300.0, marco());
        assert_eq!(e.x, x);
    }

    #[test]
    fn el_visor_solo_se_ofrece_para_lo_que_sabe_abrir() {
        assert!(se_abre("apuntes.docx"));
        assert!(se_abre("libro.epub"));
        assert!(se_abre("pagina.html"));
        assert!(se_abre("notas.md"));
        assert!(!se_abre("plano.pdf"), "el PDF lo abre su propio lector");
        assert!(!se_abre("foto.png"));
        assert!(!se_abre("apuntes.doc"), "un Word antiguo no se lee");
        assert!(!se_abre(""));
    }

    #[test]
    fn el_tamano_base_crece_con_el_tanto_por_ciento() {
        let mut a = lectura::Ajustes::default();
        a.tamano = 100;
        assert!((tamano_base(&a) - 16.0).abs() < 0.01);
        a.tamano = 200;
        assert!((tamano_base(&a) - 32.0).abs() < 0.01);
    }

    #[test]
    fn la_columna_de_lectura_no_se_estira_en_un_monitor_ancho() {
        let m = marco();
        assert_eq!(columna_que_cabe(Marco { ancho: 3840.0, ..m }), COLUMNA_MAXIMA);
        assert!(columna_que_cabe(Marco { ancho: 800.0, ..m }) < 800.0);
        assert_eq!(
            columna_que_cabe(Marco { ancho: 120.0, ..m }),
            COLUMNA_MINIMA,
            "por estrecha que sea la ventana la columna tiene un minimo"
        );
    }

    #[test]
    fn con_tinta_la_columna_es_la_de_cuando_se_anoto_y_no_la_de_la_ventana() {
        let mut e = estado_de_prueba();
        e.ajustes.columna = 612;
        let m = marco();
        assert_eq!(columna_para(&e, m), 612.0);
        e.ajustes.columna = 0;
        assert_eq!(columna_para(&e, m), COLUMNA_MAXIMA);
    }

    #[test]
    fn leer_no_se_sale_del_documento() {
        let mut e = estado_de_prueba();
        let m = marco();
        e.y = -100.0;
        acotar(&mut e, m);
        assert_eq!(e.y, 0.0, "no se sube por encima del principio");
        e.y = 1e9;
        acotar(&mut e, m);
        assert!(e.y < e.alto_doc, "no se baja mas alla del final");
    }

    #[test]
    fn en_un_monitor_ancho_la_columna_queda_en_medio() {
        let mut e = estado_de_prueba();
        let m = marco();
        acotar(&mut e, m);
        // Columna de 800 en una ventana de 1600: 400 de aire a cada lado.
        assert_eq!(e.x, -400.0);
    }

    #[test]
    fn sin_tinta_no_se_aleja_de_mas_y_con_tinta_hasta_ver_los_dos_margenes() {
        let mut e = estado_de_prueba();
        let m = Marco {
            ancho: 1000.0,
            ..marco()
        };
        e.zoom = 0.2;
        acotar(&mut e, m);
        assert_eq!(e.zoom, 1.0, "sin margenes lo mas lejos es la columna entera");
        e.ajustes.columna = 800;
        e.zoom = 0.2;
        acotar(&mut e, m);
        let esperado = 1000.0 / vista::ancho_con_margenes(800.0);
        assert!((e.zoom - esperado).abs() < 1e-4, "{} != {esperado}", e.zoom);
    }

    #[test]
    fn acercar_con_la_rueda_deja_quieto_lo_que_hay_bajo_el_raton() {
        let mut e = estado_de_prueba();
        let m = marco();
        acotar(&mut e, m);
        e.y = 3000.0;
        e.raton = (800.0, 300.0);
        let antes = en_el_documento(&e, m);
        acercar(&mut e, 2.0, m);
        let despues = en_el_documento(&e, m);
        assert!(e.zoom > 1.0);
        assert!((antes.x - despues.x).abs() < 0.5 && (antes.y - despues.y).abs() < 0.5);
    }

    #[test]
    fn anotar_la_primera_vez_fija_la_columna_y_la_letra() {
        let dir = std::env::temp_dir().join("pixpin-visor-fijar");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("tema.docx");
        let mut e = estado_de_prueba();
        let m = marco();
        alternar_anotar(&mut e, &ruta, m);
        assert!(e.anotando);
        assert_eq!(e.ajustes.columna, COLUMNA_MAXIMA as u32);
        assert!(e.ajustes.letra_fijada());
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let tam = e.ajustes.tamano;
        cambiar_letra(&mut e, &textos, &ruta, |a| a.tamano = 200);
        assert_eq!(e.ajustes.tamano, tam, "con tinta la letra no cambia");
        assert!(e.aviso.is_some(), "y se dice por que");
        assert_eq!(lectura::leer(&ruta).columna, e.ajustes.columna, "queda guardado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// K16, la queja del usuario: «si varia el tamano de muestra se moverian
    /// todas las anotaciones». Se anota con una ventana; con otra ventana,
    /// otro aumento o intentando otra letra, el parrafo anotado cae en el
    /// mismo sitio del documento, y en la pantalla texto y tinta van juntos.
    #[test]
    fn con_tinta_ni_la_ventana_ni_el_aumento_ni_la_letra_mueven_el_texto_bajo_ella() {
        use pixpin_render::lectura::Medida;
        let dir = std::env::temp_dir().join(format!("pixpin-visor-k16-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("tesis.docx");
        // Ocho unidades por letra, renglones de la letra por su interlineado.
        let mide = |t: &str, tam: f32, ancho: f32, _: &[Tramo], l: &Letra| {
            let renglones = ((t.chars().count() as f32 * 8.0) / ancho.max(1.0)).ceil().max(1.0);
            Medida { ancho: ancho.min(t.chars().count() as f32 * 8.0), alto: renglones * tam * l.interlineado, minimo: 40.0, renglones: Vec::new() }
        };
        let ancha = marco();
        let estrecha = Marco { ancho: 700.0, alto: 600.0, area: pixpin_geom::Rect { x: 0, y: 0, ancho: 700, alto: 600 }, ..marco() };
        let mut e = estado_de_prueba();
        e.doc = documento_de_muestra();
        // Se anota con la ventana estrecha: la columna es la que cabia en ella.
        alternar_anotar(&mut e, &ruta, estrecha);
        let fijada = e.ajustes.columna as f32;
        assert_eq!(fijada, columna_que_cabe(estrecha).round());
        let donde = |e: &Estado, m: Marco| {
            let (c, _) = colocar(&e.doc, &e.ajustes, columna_para(e, m), e.hoja, &mide);
            c.iter().map(|x| (x.bloque, x.sangria, x.y)).collect::<Vec<_>>()
        };
        let antes = donde(&e, estrecha);
        assert_eq!(donde(&e, ancha), antes, "otra ventana no recoloca el texto");
        // Un trazo sobre el quinto bloque: en la pantalla va con su parrafo a
        // cualquier aumento (la misma transformada para los dos).
        let (_, x5, y5) = antes[5];
        let trazo = Punto2::nuevo(x5 + 20.0, y5 + 5.0);
        for zoom in [0.5f32, 1.0, 1.8] {
            e.zoom = zoom;
            let s = px(&e, ancha);
            let pantalla = |x: f32, y: f32| ((x - e.x) * s, (y - e.y) * s);
            let (px_t, py_t) = pantalla(trazo.x, trazo.y);
            let (px_p, py_p) = pantalla(x5, y5);
            assert!((px_t - px_p - 20.0 * s).abs() < 1e-3 && (py_t - py_p - 5.0 * s).abs() < 1e-3);
        }
        // Otra letra con tinta encima: no, y se dice por que.
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let guardados = e.ajustes.clone();
        cambiar_letra(&mut e, &textos, &ruta, |a| a.tamano = 150);
        cambiar_letra(&mut e, &textos, &ruta, |a| a.tipo = 3);
        assert_eq!(e.ajustes, guardados);
        assert!(e.aviso.is_some());
        assert_eq!(donde(&e, ancha), antes);
        // Caso negativo: sin tinta (columna suelta) la ventana si recoloca.
        e.ajustes.columna = 0;
        assert_ne!(donde(&e, ancha), donde(&e, estrecha), "sin columna fijada cada ventana tiene la suya");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_trazo_del_raton_cae_en_unidades_del_documento() {
        let mut e = estado_de_prueba();
        let m = Marco { e: 2.0, ..marco() };
        e.x = -100.0;
        e.y = 500.0;
        e.zoom = 1.5;
        e.raton = (300.0, 30.0);
        let q = en_el_documento(&e, m);
        // 3 pixeles por unidad: 300 px son 100 unidades desde -100.
        assert!((q.x - 0.0).abs() < 1e-4 && (q.y - 510.0).abs() < 1e-4);
    }

    #[test]
    fn el_marcador_va_a_la_fraccion_del_documento_donde_se_puso() {
        let dir = std::env::temp_dir().join("pixpin-visor-marcador");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("libro.epub");
        let mut e = estado_de_prueba();
        e.y = 2500.0;
        poner_marcador(&mut e, 1, &ruta);
        assert_eq!(e.ajustes.marcadores.len(), 1);
        assert_eq!(e.ajustes.marcadores[0].emoji, "⭐");
        e.y = 0.0;
        ir_al_marcador(&mut e, 0);
        assert!((e.y - 2500.0).abs() < 0.5);
        ir_al_marcador(&mut e, 7);
        assert!((e.y - 2500.0).abs() < 0.5, "un marcador que no existe no mueve nada");
        let _ = std::fs::remove_dir_all(&dir);
    }

    pub(super) fn colocado(texto: &str, y: f32) -> Colocado {
        Colocado {
            texto: texto.into(),
            tramos: Vec::new(),
            clase: Clase::Parrafo,
            tam: 16.0,
            sangria: 0.0,
            ancho: 800.0,
            y,
            alto: 100.0,
            color: TEXTO,
            bloque: None,
            caja: None,
            letra: Letra::del_cuerpo(Hoja::Word, &lectura::Ajustes::default()),
            recuadro: false,
            renglones: Vec::new(),
        }
    }

    #[test]
    fn buscar_encuentra_en_lo_pintado_y_empieza_por_lo_que_se_lee() {
        let mut e = estado_de_prueba();
        e.colocados = vec![
            colocado("• Un hidalgo en la lista", 0.0),
            colocado("nada", 100.0),
            colocado("otro Hidalgo", 200.0),
        ];
        e.hallar.caja.tecla(VK_F, false, true);
        for c in "hidalgo".chars() {
            assert!(caracter_de_las_cajas(&mut e, c), "con la caja abierta, las letras son suyas");
        }
        assert_eq!(e.hallar.caja.busqueda.coincidencias.len(), 2);
        assert_eq!(e.hallar.caja.busqueda.cuenta(), Some((1, 2)));
        assert!(e.hallar.saltar, "encontrar lleva la vista a la actual");
        // Leyendo mas abajo, la actual es la primera desde ahi.
        e.y = 150.0;
        rebuscar(&mut e);
        assert_eq!(e.hallar.caja.busqueda.la_actual().map(|c| c.texto), Some(2));
        // Siguiente da la vuelta al principio.
        hecho_de_buscar(&mut e, crate::buscador::Hecho::Siguiente);
        assert_eq!(e.hallar.caja.busqueda.la_actual().map(|c| c.texto), Some(0));
    }

    #[test]
    fn con_la_caja_cerrada_las_letras_no_son_de_buscar() {
        let mut e = estado_de_prueba();
        e.colocados = vec![colocado("hola", 0.0)];
        assert!(!caracter_de_las_cajas(&mut e, 'h'));
        assert!(!escribiendo(&e));
        assert!(e.hallar.caja.busqueda.consulta.is_empty());
    }

    #[test]
    fn el_indice_lleva_al_bloque_de_su_titulo() {
        let mut e = estado_de_prueba();
        e.doc.bloques = vec![
            Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("a")]),
            Bloque::nuevo(Clase::Titulo(1), vec![Trozo::llano("Dos")]),
        ];
        e.indice = indice::de(&e.doc);
        let colocar = |y, bloque| Colocado {
            texto: "x".into(),
            tramos: Vec::new(),
            clase: Clase::Parrafo,
            tam: 16.0,
            sangria: 0.0,
            ancho: 800.0,
            y,
            alto: 20.0,
            color: TEXTO,
            bloque,
            caja: None,
            letra: Letra::del_cuerpo(Hoja::Word, &lectura::Ajustes::default()),
            recuadro: false,
            renglones: Vec::new(),
        };
        e.colocados = vec![colocar(10.0, None), colocar(40.0, Some(0)), colocar(900.0, Some(1))];
        assert_eq!(y_del_bloque(&e, 1), 900.0);
        e.y = 950.0;
        assert_eq!(bloque_arriba(&e), 1);
        e.y = 0.0;
        assert_eq!(bloque_arriba(&e), 0, "antes del primer bloque, el primero");
    }

    /// Un documento de muestra: titulo, capitulos, parrafos y una lista.
    fn documento_de_muestra() -> Documento {
        let p = |t: &str| Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(t)]);
        let texto = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho \
                     tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua, \
                     rocín flaco y galgo corredor.";
        let mut bloques = vec![Bloque::nuevo(Clase::Titulo(1), vec![Trozo::llano("Capítulo primero")])];
        for _ in 0..3 {
            bloques.push(p(texto));
        }
        bloques.push(Bloque::nuevo(Clase::Titulo(2), vec![Trozo::llano("Que trata de la condición")]));
        bloques.push(Bloque::nuevo(Clase::Lista, vec![Trozo::llano("una olla de algo más vaca que carnero")]));
        bloques.push(Bloque::nuevo(Clase::Lista, vec![Trozo::llano("salpicón las más noches")]));
        for _ in 0..6 {
            bloques.push(p(texto));
        }
        Documento {
            titulo: "Don Quijote".into(),
            autor: "Miguel de Cervantes".into(),
            bloques,
            imagenes: Vec::new(),
        }
    }

    /// **El lector pintado de verdad**, en PNG, para mirarlo: leyendo con el
    /// indice abierto, y anotando alejado hasta ver la columna con sus
    /// margenes, con tinta en el texto y en el margen. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lector -- --ignored
    /// --nocapture`. Deja los PNG en `PIXPIN_MUESTRAS` o en la temporal.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lector_de_documentos() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let m = Marco {
            ancho: 1400.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect { x: 0, y: 0, ancho: 1400, alto: 900 },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1400, 900).expect("superficie");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);

        let mut e = estado_de_prueba();
        e.doc = documento_de_muestra();
        e.indice = indice::de(&e.doc);
        e.alto_doc = 0.0;
        e.ajustes.marcadores = lectura::con_marcador(&[], 0.1, "⭐", 1);
        e.ajustes.marcadores = lectura::con_marcador(&e.ajustes.marcadores, 0.6, "💡", 2);

        let foto = |e: &mut Estado, nombre: &str| {
            motor
                .dibujar(&fuera.destino, |p| {
                    medir(e, p, m);
                    acotar(e, m);
                    pintar(e, p, m, &textos);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let ruta = carpeta.join(format!("lector-docs-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
        };

        // Leyendo, con el indice y la pastilla.
        e.viendo_indice = true;
        e.pastilla_hasta = u64::MAX;
        foto(&mut e, "indice");

        // Buscando «hidalgo» (D9): todas marcadas y la actual en naranja; y
        // cambiando el nombre (D5).
        e.viendo_indice = false;
        e.pastilla_hasta = 0;
        e.hallar.caja.tecla(VK_F, false, true);
        for c in "HIDALGO".chars() {
            caracter_de_las_cajas(&mut e, c);
        }
        hecho_de_buscar(&mut e, crate::buscador::Hecho::Siguiente);
        foto(&mut e, "buscando");
        e.hallar.caja.tecla(0x1B, false, false);
        e.nombre.visto = "Don Quijote.epub".into();
        e.nombre.empezar();
        caracter_de_las_cajas(&mut e, ' ');
        caracter_de_las_cajas(&mut e, 'I');
        foto(&mut e, "renombrando");
        e.nombre.editando = None;

        // Anotando: alejado al minimo (la columna con sus dos margenes), un
        // subrayado en el texto, una nota en el margen y un resaltado.
        e.viendo_indice = false;
        let ruta = std::env::temp_dir().join("pixpin-muestra-lector.docx");
        alternar_anotar(&mut e, &ruta, m);
        e.zoom = 0.1;
        acotar(&mut e, m);
        // Con las herramientas del lienzo: la tinta negra de siempre sale
        // clara sobre el papel oscuro del lector (`dibujo::tema`), el rojo
        // del panel se queda rojo y el resaltador cubre el renglon.
        use pixpin_motor2d::gesto::Herramienta;
        let trazo = |e: &mut Estado, h: Herramienta, rojo: bool, puntos: &[(f32, f32)]| {
            crate::dibujo::teclas::elegir_herramienta(&mut e.tinta.gesto, h);
            if rojo {
                e.tinta.gesto.estilo.trazo = pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19);
            }
            let v: Vec<Punto2> = puntos.iter().map(|q| Punto2::nuevo(q.0, q.1)).collect();
            e.tinta.trazar(&mut e.capa, &v);
        };
        let fin = e.columna + 60.0;
        trazo(&mut e, Herramienta::Lapiz, false, &[(0.0, 180.0), (200.0, 182.0), (420.0, 181.0)]);
        trazo(
            &mut e,
            Herramienta::Lapiz,
            true,
            &[(fin, 170.0), (fin + 80.0, 150.0), (fin + 160.0, 170.0), (fin + 240.0, 150.0)],
        );
        trazo(&mut e, Herramienta::Rectangulo, true, &[(fin, 300.0), (fin + 200.0, 380.0)]);
        trazo(&mut e, Herramienta::Resaltador, false, &[(0.0, 250.0), (300.0, 250.0)]);
        foto(&mut e, "anotando-alejado");
        let _ = std::fs::remove_file(lectura::ruta_de_ajustes(&ruta));
    }

    /// **La tinta del movil sobre el Word del usuario, en el PC** (K16): una
    /// copia del Word (`PIXPIN_DOCX`), la tinta de su mensaje
    /// (`PIXPIN_TINTA`, `anot-<uid>.excalidraw`) y su maqueta
    /// (`PIXPIN_MAQUETA`, `columna,izq,der,t,g,l`), pintados como en el lector
    /// a dos anchos de ventana y dos aumentos, en PNG para mirarlos junto a la
    /// pagina del movil. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_tinta_del_movil -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y una copia de los datos del usuario"]
    fn muestra_de_la_tinta_del_movil_en_el_pc() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let docx = std::env::var("PIXPIN_DOCX").expect("PIXPIN_DOCX");
        let tinta = std::env::var("PIXPIN_TINTA").expect("PIXPIN_TINTA");
        let maqueta = pixpin_sincro::anotado::Maqueta::de_texto(&std::env::var("PIXPIN_MAQUETA").expect("PIXPIN_MAQUETA"))
            .expect("maqueta");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut e = estado_de_prueba();
        e.doc = pixpin_docs::abrir(Path::new(&docx)).expect("el Word se lee");
        e.alto_doc = 0.0;
        e.ajustes.columna = maqueta.columna as u32;
        e.ajustes.tamano = maqueta.tamano as u32;
        e.ajustes.tipo = crate::anotado_del_adjunto::tipo_del_movil(maqueta.tipo);
        e.ajustes.grosor = crate::anotado_del_adjunto::grosor_del_movil(maqueta.grosor);
        // La tinta del movil cuenta desde el borde de su pagina: se corre su `izq`.
        e.capa = Capa::leer_corrida(Path::new(&tinta), maqueta.izq as f32);
        let caja_tinta = e.capa.escena.caja().expect("hay tinta");
        println!("tinta (columna): {caja_tinta:?}");
        let foto = |e: &mut Estado, nombre: &str, ancho: u32, alto: u32, zoom: f32| {
            let m = Marco {
                ancho: ancho as f32,
                alto: alto as f32,
                e: 1.0,
                escala_por_cien: 100,
                area: pixpin_geom::Rect { x: 0, y: 0, ancho, alto },
            };
            let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
            e.zoom = zoom;
            motor
                .dibujar(&fuera.destino, |p| {
                    medir(e, p, m);
                    // La tinta en medio de la ventana.
                    let s = px(e, m);
                    e.x = (caja_tinta.0 + caja_tinta.2) / 2.0 - m.ancho / s / 2.0;
                    e.y = (caja_tinta.1 + caja_tinta.3) / 2.0 - m.alto / s / 2.0;
                    acotar(e, m);
                    pintar(e, p, m, &textos);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (w, h, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba { ancho: w, alto: h, pixeles })
                .expect("png");
            let ruta = carpeta.join(format!("k16-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
            // Donde cae la fila de la referencia 37 en esta ventana y la tinta:
            // con la misma transformada, el texto y la tinta se mueven juntos.
            let fila = e.colocados.iter().find(|c| c.texto.starts_with("Álvarez Ochoa")).map(|c| c.y);
            println!("  {nombre}: fila 37 en y {fila:?}; alto del documento {}", e.alto_doc);
        };
        foto(&mut e, "word-1400", 1400, 900, 1.0);
        foto(&mut e, "word-900", 900, 700, 1.0);
        foto(&mut e, "word-1400-cerca", 1400, 900, 1.8);
    }

    /// **La tinta de antes de K16 llevada a la maqueta del movil**, con el
    /// libro del usuario (una copia: `PIXPIN_LIBRO`, con su
    /// `.pixpin-lectura` de la version 1 y su `.pixpin-anotado` al lado).
    /// Deja un PNG donde empieza lo anotado para mirar que cada trazo sigue
    /// sobre su renglon. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_tinta_de_antes -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y una copia de los datos del usuario"]
    fn muestra_de_la_tinta_de_antes_mudada() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let libro = PathBuf::from(std::env::var("PIXPIN_LIBRO").expect("PIXPIN_LIBRO"));
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut e = estado_de_prueba();
        e.doc = pixpin_docs::abrir(&libro).expect("el libro se lee");
        e.hoja = Hoja::de(&libro);
        e.alto_doc = 0.0;
        e.ajustes = lectura::leer(&libro);
        assert!(e.ajustes.de_antes, "la copia tiene que traer su fichero de la version 1");
        e.capa = Capa::leer(&lector_tinta::ruta_de_capa(&libro));
        e.tinta_de_antes = e.ajustes.letra_fijada() && !e.capa.vacia();
        let antes: Vec<(f32, f32)> = e.capa.escena.visibles().map(|x| (x.caja().0, x.caja().1)).collect();
        let m = Marco {
            ancho: 1400.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect { x: 0, y: 0, ancho: 1400, alto: 900 },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1400, 900).expect("superficie");
        for (k, nombre) in ["libro-mudado-1", "libro-mudado-2"].iter().enumerate() {
            motor
                .dibujar(&fuera.destino, |p| {
                    let t = std::time::Instant::now();
                    medir(&mut e, p, m);
                    println!("medir {} bloques (y mudar): {:.0} ms", e.doc.bloques.len(), t.elapsed().as_secs_f64() * 1000.0);
                    let cajas: Vec<(f32, f32, f32, f32)> = e.capa.escena.visibles().map(|x| x.caja()).collect();
                    // La primera tanda de trazos, y la del medio.
                    let c = cajas[(cajas.len() / 2) * k];
                    e.zoom = 0.6;
                    let s = px(&e, m);
                    e.x = (c.0 + c.2) / 2.0 - m.ancho / s / 2.0;
                    e.y = c.1 - m.alto / s / 3.0;
                    acotar(&mut e, m);
                    pintar(&mut e, p, m, &textos);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (w, h, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba { ancho: w, alto: h, pixeles })
                .expect("png");
            let ruta = carpeta.join(format!("k16-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
        }
        let despues: Vec<(f32, f32)> = e.capa.escena.visibles().map(|x| (x.caja().0, x.caja().1)).collect();
        for (a, b) in antes.iter().zip(&despues).take(8) {
            println!("  {a:?} -> {b:?}");
        }
        assert!(!e.ajustes.de_antes && e.guardar_ya, "mudada una vez y a guardar ya");
    }

    /// **La latencia de la tinta en el lector**, medida sin ventana como la
    /// del lienzo (`ventana_editor/medir.rs`): lo que tarda un fotograma
    /// entero del lector anotando —el documento, 300 trazos ya hechos, el
    /// trazo vivo creciendo, la barra y el panel— desde que llega el punto
    /// hasta que la GPU termino. En el lector la tinta va en la MISMA pasada
    /// que el texto (no puede temblar al desplazar), asi que eso es lo que
    /// cuesta cada punto. Puerta: cabe en un fotograma de 60 Hz.
    ///
    /// ```text
    /// cargo test --release -p pixpin --bin pixpinmax fotograma_del_lector_anotando -- --ignored --nocapture --test-threads=1
    /// ```
    #[test]
    #[ignore = "necesita GPU real; puerta: ejecutar en --release con --ignored --nocapture"]
    fn un_fotograma_del_lector_anotando_cabe_en_un_fotograma_de_60_hz() {
        use pixpin_motor2d::gesto::{EventoGesto, Herramienta};
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let m = Marco {
            ancho: 1920.0,
            alto: 1080.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect { x: 0, y: 0, ancho: 1920, alto: 1080 },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1920, 1080).expect("superficie");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut e = estado_de_prueba();
        e.doc = documento_de_muestra();
        e.alto_doc = 0.0;
        let ruta = std::env::temp_dir().join(format!("pixpin-banco-lector-{}.docx", std::process::id()));
        alternar_anotar(&mut e, &ruta, m);
        let fotograma = |e: &mut Estado| {
            let t = std::time::Instant::now();
            motor
                .dibujar(&fuera.destino, |p| {
                    medir(e, p, m);
                    acotar(e, m);
                    pintar(e, p, m, &textos);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("GPU");
            t.elapsed().as_secs_f64() * 1000.0
        };
        // Solo el documento, para saber que parte es de la tinta.
        let mut solo_doc: Vec<f64> = Vec::new();
        let _ = fotograma(&mut e);
        for _ in 0..60 {
            solo_doc.push(fotograma(&mut e));
        }
        solo_doc.sort_by(|a, b| a.total_cmp(b));
        println!(
            "lector sin tinta: media {:.2} ms | p95 {:.2} ms",
            solo_doc.iter().sum::<f64>() / solo_doc.len() as f64,
            solo_doc[solo_doc.len() * 95 / 100]
        );
        // 300 trazos ya hechos por la pagina, de 40 puntos cada uno.
        crate::dibujo::teclas::elegir_herramienta(&mut e.tinta.gesto, Herramienta::Lapiz);
        for i in 0..300 {
            let y = 40.0 + (i % 60) as f32 * 12.0;
            let x0 = (i / 60) as f32 * 160.0;
            let v: Vec<Punto2> = (0..40)
                .map(|k| Punto2::nuevo(x0 + k as f32 * 3.0, y + (k as f32 * 0.4).sin() * 4.0))
                .collect();
            e.tinta.trazar(&mut e.capa, &v);
        }
        // El primero mide el documento y tesela lo quieto: no cuenta.
        let primero = fotograma(&mut e);
        let camara = pixpin_motor2d::camara::Camara::nueva();
        let mut p = Punto2::nuevo(200.0, 300.0);
        e.tinta.mano.al_motor(
            EventoGesto::Pulsar {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut e.tinta.gesto,
            &mut e.capa.escena,
            &camara,
        );
        let mut tiempos = Vec::new();
        for k in 0..240 {
            // Tres puntos por fotograma: un raton de 125 Hz a 60 Hz de pantalla.
            for _ in 0..3 {
                p = Punto2::nuevo(p.x + 2.0, 300.0 + (k as f32 * 0.2).sin() * 30.0);
                e.tinta.mano.al_motor(
                    EventoGesto::Mover {
                        p,
                        shift: false,
                        alt: false,
                        presion: None,
                    },
                    &mut e.tinta.gesto,
                    &mut e.capa.escena,
                    &camara,
                );
            }
            tiempos.push(fotograma(&mut e));
        }
        tiempos.sort_by(|a, b| a.total_cmp(b));
        let media = tiempos.iter().sum::<f64>() / tiempos.len() as f64;
        let p95 = tiempos[tiempos.len() * 95 / 100];
        let peor = *tiempos.last().unwrap();
        println!(
            "lector anotando 1920x1080, 300 trazos + trazo vivo de hasta 720 puntos: primero {primero:.2} ms | media {media:.2} ms | p95 {p95:.2} ms | peor {peor:.2} ms"
        );
        let _ = std::fs::remove_file(lectura::ruta_de_ajustes(&ruta));
        assert!(p95 < 16.7, "el p95 ({p95:.2} ms) no cabe en un fotograma de 60 Hz");
    }
}
