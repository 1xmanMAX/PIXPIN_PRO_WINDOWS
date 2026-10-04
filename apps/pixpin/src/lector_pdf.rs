//! **El lector de PDF**: todas las hojas una debajo de otra, a pantalla
//! completa, para leer con la rueda y anotar encima de cualquiera sin crear
//! ningun proyecto. Puerto de `pdf/LectorPdfActivity.kt` del movil
//! (v0.60 el lector ligero, v0.69-v0.70 anotar sobre todas las hojas, v0.78
//! y v0.85 los espacios para anotar a boton, v0.81 los marcadores).
//!
//! # Con que se pinta
//!
//! Las hojas las dibuja **`Windows.Data.Pdf`** (ya en `pixpin-pdf`): viene
//! con Windows, no anade un byte y es lo que ya usan los pines y el paso de
//! PDF a proyecto. Se descarto WebView2 para esto: el visor de PDF de Edge
//! se ve bien, pero no deja poner la tinta **en la misma pasada** que la
//! hoja (seria otra vez el fallo de la tinta que va un fotograma por detras)
//! y cuesta un proceso del navegador por documento.
//!
//! Rasterizar una hoja son decenas de milisegundos, asi que va **en un hilo
//! aparte** con su propio `Documento` (el de WinRT no cruza de hilo): la
//! ventana le pide las hojas que se ven, por orden, y las pinta en cuanto
//! llegan; mientras, un hueco del tamano justo, porque las medidas de todas
//! las hojas se piden al abrir y el desplazamiento no salta. Mientras la
//! rueda cambia el aumento **no se rasteriza nada** (se estira lo que hay,
//! que es gratis) y al parar se pide la hoja a la resolucion nueva: es lo
//! que el movil aprendio con el pellizco.
//!
//! # Las unidades y la tinta
//!
//! Cada hoja mide `vista::ANCHO_HOJA` (1400, el `PAGE_WIDTH` del movil) y
//! tiene **su propio dibujo**, con el cero en su esquina: los margenes para
//! anotar son las equis negativas y las de mas alla de 1400. La hoja y su
//! tinta se pintan con la misma transformada, asi que ni desplazar ni
//! acercar las separa. Cada dibujo va a `<nombre>.pdf.pixpin-anotado/
//! hoja-<n>.excalidraw` (ver `lector_tinta`).
//!
//! # Teclas y raton
//!
//! Rueda: pasar hojas. Ctrl+rueda: acercar o alejar hacia el raton.
//! Mayus+rueda o la rueda de lado: a los margenes. Arrastrar (izquierdo
//! leyendo, central siempre): mover. Flechas, AvPag/RePag, Espacio,
//! Inicio/Fin. Ctrl+0 aumento normal. Ctrl+[ y Ctrl+] (o los mandos de los
//! lados, abajo, como en el movil): espacio para anotar a cada lado. `L`
//! escuchar en voz alta (`voz`).
//! `M` marcador, Ctrl+1..9 ir, clic derecho en su punto: quitar. `A`
//! anotar (P lapiz, R resaltador, E goma, 1-5 color, Ctrl+Z/Ctrl+Y). `G`
//! ajustes. Esc salir.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_docs::{lectura, vista};
use pixpin_motor2d::marcas::{self, Marca};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::riel_marcas::DestinoRiel;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::lector::{
    self, APAGADO, CRISTAL, DORADO, EMOJIS, FONDO_PDF, PAPEL_DEL_MARGEN, RAYA, TEXTO, ahora_ms,
    con_alfa, dentro,
};
use crate::lector_tinta::{self, Capa, Tinta};
use crate::overlay::Recursos;

mod voz;

/// La pastilla del nombre se va sola tras esto.
const MS_DE_LA_PASTILLA: u64 = 2600;
/// Tras esto sin tocar, lo anotado se escribe a disco solo.
const MS_PARA_GUARDAR: u64 = 1500;
/// Tras cambiar el aumento, lo que se espera antes de volver a rasterizar:
/// dos muescas seguidas no piden dos rasterizados (el respiro del movil).
const MS_ZOOM_FIRME: u64 = 180;
/// El tope de pixeles de una hoja rasterizada: un A1 a cuatro veces el
/// ancho de la pantalla serian cincuenta megas por hoja
/// (`PIXELES_POR_HOJA` del movil). Con el tope la hoja sale a menos
/// aumento del pedido, pero sale.
const PIXELES_POR_HOJA: f32 = 16_000_000.0;
/// Los anchos de rasterizado van por escalones: sin ellos cada muesca de la
/// rueda pediria otra hoja de un pixel mas.
const ESCALON: u32 = 256;
/// Hojas que se guardan pintadas por encima y por debajo de las que se ven.
const HOJAS_DE_RESERVA: usize = 3;
/// Aire arriba y abajo del documento, en unidades: una hoja sola no queda
/// pegada al borde (el hueco del 28 % del movil con una hoja).
const AIRE: f32 = 60.0;
/// Lo mas que se aleja: se ven varias hojas a la vez. En el movil sin
/// espacios no se aleja por debajo de 1 porque 1 ya es la hoja de borde a
/// borde; en un monitor apaisado 1 es una hoja en medio, y alejar para ver
/// por donde se va es de lo mas util.
const ZOOM_MINIMO_PDF: f32 = 0.3;

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
const VK_G: u32 = 0x47;
const VK_F: u32 = 0x46;
const VK_F2: u32 = 0x71;
const VK_F3: u32 = 0x72;
const VK_M: u32 = 0x4D;
const VK_L: u32 = 0x4C;
const VK_S: u32 = 0x53;
const VK_Y: u32 = 0x59;
const VK_Z: u32 = 0x5A;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;
/// `[` y `]` en un teclado espanol o ingles (`VK_OEM_4` y `VK_OEM_6`).
const VK_CORCHETE_ABRE: u32 = 0xDB;
const VK_CORCHETE_CIERRA: u32 = 0xDD;

/// Abre el lector con ese PDF, en su propio hilo (como el visor).
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("lector-pdf".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = crate::dispositivo_perdido::con_recursos("lector_pdf", |r| {
                abrir(r, &textos, &ubicacion, &ruta)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el lector de PDF");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del lector de PDF");
    }
}

/// Si esto lo abre el lector de PDF.
pub fn se_abre(nombre: &str) -> bool {
    pixpin_docs::extension(nombre) == "pdf"
}

// ---------------------------------------------------------------------------
// El hilo que rasteriza

enum Pedido {
    /// Las hojas que hacen falta, por orden: sustituye a la lista anterior
    /// (lo que ya no se ve no se pinta).
    Quiero(Vec<(usize, u32)>),
    /// El PDF entero con lo anotado: las ordenes de cada hoja que tiene
    /// algo, lo que se abre a cada lado y donde escribirlo.
    Exportar {
        tinta: HashMap<usize, Vec<Orden>>,
        altos: Vec<f32>,
        espacios: (f32, f32),
        /// Los marcadores, que van al indice del PDF.
        marcas: Vec<Marca>,
        destino: PathBuf,
    },
}

enum Llega {
    Medidas(Vec<(f32, f32)>),
    Hoja(usize, ImagenRgba),
    NoSeAbre(String),
    Progreso(usize, usize),
    Exportado(std::result::Result<PathBuf, String>),
}

fn hilo_de_hojas(
    ruta: PathBuf,
    pedidos: mpsc::Receiver<Pedido>,
    salida: mpsc::Sender<Llega>,
    ventana: isize,
) {
    let avisar = |l: Llega| {
        let _ = salida.send(l);
        pixpin_shell::overlay::despertar(ventana);
    };
    let documento = match pixpin_pdf::Documento::abrir(&ruta) {
        Ok(d) => d,
        Err(e) => {
            avisar(Llega::NoSeAbre(e.to_string()));
            return;
        }
    };
    avisar(Llega::Medidas(documento.medidas()));
    let mut cola: std::collections::VecDeque<(usize, u32)> = Default::default();
    loop {
        // Sin trabajo se duerme hasta el siguiente pedido; con trabajo solo
        // mira si ha llegado una lista mas nueva.
        let primero = if cola.is_empty() {
            match pedidos.recv() {
                Ok(p) => Some(p),
                Err(_) => return,
            }
        } else {
            None
        };
        for p in primero
            .into_iter()
            .chain(std::iter::from_fn(|| pedidos.try_recv().ok()))
        {
            match p {
                Pedido::Quiero(lista) => cola = lista.into(),
                Pedido::Exportar {
                    tinta,
                    altos,
                    espacios,
                    marcas,
                    destino,
                } => {
                    let hecho = exportar(
                        &documento,
                        &tinta,
                        &altos,
                        espacios,
                        &marcas,
                        &destino,
                        &|n, t| avisar(Llega::Progreso(n, t)),
                    );
                    avisar(Llega::Exportado(hecho.map(|_| destino)));
                }
            }
        }
        if let Some((i, ancho)) = cola.pop_front() {
            match documento.renderizar(i as u32, ancho) {
                Ok(img) => avisar(Llega::Hoja(i, img)),
                Err(e) => tracing::warn!(?e, hoja = i, "hoja del PDF que no se pudo dibujar"),
            }
        }
    }
}

/// **Una hoja del PDF con lo anotado**, lista para cualquier formato: la
/// pagina como imagen (id `i + 1`: el cero lo reserva el escritor de PDF
/// para «sin imagen») a su ancho de unidades, la tinta encima y los espacios
/// para anotar que haya puestos. La usan el «Exportar» de aqui y la hoja de
/// compartir (`compartir.rs`), que asi sacan la misma hoja.
pub(crate) fn hoja_anotada(
    i: usize,
    altos: &[f32],
    tinta: Option<&[Orden]>,
    (izq, der): (f32, f32),
) -> Option<pixpin_motor2d::exportar::Hoja> {
    let alto = *altos.get(i)?;
    let mut ordenes = vec![Orden::Imagen {
        id_objeto: i as u64 + 1,
        x: 0.0,
        y: 0.0,
        ancho: vista::ANCHO_HOJA,
        alto,
        opacidad: 1.0,
        recorte: None,
        angulo: 0.0,
    }];
    ordenes.extend(tinta.unwrap_or_default().iter().cloned());
    Some(pixpin_motor2d::exportar::Hoja {
        nombre: String::new(),
        caja: (-izq, 0.0, vista::ANCHO_HOJA + der, alto),
        ordenes,
        marcos: Vec::new(),
        granos: Vec::new(),
        grafitos: Vec::new(),
    })
}

/// **El PDF con lo anotado** (el «Exportar» del lector del movil,
/// `ExportarPdfAnotado`): cada hoja dibujada a su ancho de unidades, con su
/// tinta encima y con los espacios para anotar que haya puestos, escrito con
/// el `pixpin_pdf::escribir` de siempre.
fn exportar(
    documento: &pixpin_pdf::Documento,
    tinta: &HashMap<usize, Vec<Orden>>,
    altos: &[f32],
    (izq, der): (f32, f32),
    marcas: &[Marca],
    destino: &Path,
    progreso: &dyn Fn(usize, usize),
) -> std::result::Result<(), String> {
    let total = altos.len();
    // **El PDF de siempre con lo anotado encima** (`PdfConAnotaciones` del
    // movil): texto que se busca, vectores y el peso del original mas la
    // tinta. Solo si no se deja (cifrado, roto) va como antes, pintado.
    let limpio = std::fs::read(documento.ruta()).ok().map(|mut b| {
        if let Some(n) = pixpin_pdf::cocido::largo_sin_lo_cocido(&b) {
            b.truncate(n);
        }
        b
    });
    let indice = crate::compartir::pdf_anotado::marcadores_de(marcas);
    if let Some(bytes) = limpio
        .and_then(|b| crate::compartir::pdf_anotado::con_tinta(&b, tinta, (izq, der), &indice))
    {
        progreso(total, total);
        if let Some(carpeta) = destino.parent() {
            std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
        }
        return std::fs::write(destino, bytes).map_err(|e| e.to_string());
    }
    tracing::info!("el PDF no se deja anotar encima; se exporta pintado");
    let hojas: Vec<_> = (0..total)
        .filter_map(|i| hoja_anotada(i, altos, tinta.get(&i).map(Vec::as_slice), (izq, der)))
        .collect();
    // **Los marcadores, en el indice del PDF**: cada uno con su emoticono y
    // su hoja, y llevando a su altura, como el riel del lector.
    let indice: Vec<pixpin_pdf::escribir::Marcador> = marcas
        .iter()
        .filter_map(|m| {
            let hoja = marcas::pagina_de(m) as usize;
            let alto = *altos.get(hoja)?;
            Some(pixpin_pdf::escribir::Marcador {
                titulo: format!("{} Hoja {}", m.emoji, hoja + 1),
                hoja,
                y: marcas::alto_en_la_pagina(m) as f32 * alto,
            })
        })
        .collect();
    let hechas = std::cell::Cell::new(0usize);
    let bytes = pixpin_pdf::escribir::de_hojas_con_indice(
        &hojas,
        None,
        &|id| {
            let i = id.checked_sub(1)? as u32;
            hechas.set(hechas.get() + 1);
            progreso(hechas.get(), total);
            documento
                .renderizar(i, vista::ANCHO_HOJA as u32)
                .ok()
                .map(|img| pixpin_pdf::escribir::Pixeles {
                    ancho: img.ancho,
                    alto: img.alto,
                    rgba: img.pixeles,
                })
        },
        None,
        &indice,
    )
    .ok_or_else(|| "sin hojas".to_string())?;
    if let Some(carpeta) = destino.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    std::fs::write(destino, bytes).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// La ventana

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Engranaje,
    /// Escuchar en voz alta (`voz`).
    Escuchar,
    /// Un boton de la barra de escuchar.
    Voz(crate::leer_en_voz::BotonVoz),
    /// Un boton de los mandos de los lados.
    Lado(lector::BotonLado),
    Exportar,
    /// La hoja de compartir de toda la aplicacion, con este PDF.
    Compartir,
    AbrirCarpeta,
    QuitarMarcas,
    QuitarTinta,
    /// «Al proyecto» (`LectorPdfActivity`): el PDF pasa a ser un proyecto con
    /// lo anotado en sus hojas, que asi viaja al sincronizar.
    AlProyecto,
    /// Pulsar el nombre: cambiarlo (D5).
    Renombrar,
    /// Un boton de la caja de buscar (D9).
    Buscar(crate::buscador::Boton),
}

#[derive(Debug, Clone, Copy)]
enum Arrastre {
    Mover {
        desde: (f32, f32),
        vista: (f32, f32),
        movido: bool,
    },
    /// Anotando: el gesto entero va a la tinta (a la capa de su hoja, que
    /// guarda `Tinta::hoja`) aunque el raton se salga por abajo.
    Trazo,
}

#[derive(Debug, Clone, Copy)]
struct Marco {
    ancho: f32,
    alto: f32,
    e: f32,
    escala_por_cien: u32,
    /// La ventana en el escritorio: los eventos llegan en esas coordenadas.
    area: pixpin_geom::Rect,
}

struct Estado {
    nombre: String,
    ajustes: lectura::Ajustes,
    medidas: Vec<(f32, f32)>,
    hojas: vista::Hojas,
    /// Cada hoja ya pintada: con que ancho y su bitmap.
    pintadas: HashMap<usize, (u32, ID2D1Bitmap1)>,
    ultima_lista: Vec<(usize, u32)>,
    x: f32,
    y: f32,
    zoom: f32,
    zoom_cambiado: u64,
    marcas: Vec<Marca>,
    capas: HashMap<usize, Capa>,
    /// Donde va la capa de cada hoja: la hoja del proyecto si el PDF es de
    /// uno (y viaja al sincronizar, como en el movil), o junto al PDF.
    donde: crate::lector_pdf_proyecto::DondeVa,
    /// Las hojas tocadas, en orden, para que Ctrl+Z deshaga en la que toca.
    hechos: Vec<usize>,
    deshechos: Vec<usize>,
    anotando: bool,
    /// Las herramientas de dibujo del lienzo (`lector_tinta::Tinta`). Una para
    /// todo el PDF; cada evento va a la capa de su hoja.
    tinta: Tinta,
    /// La hoja que la tinta cambio en el gesto en curso: al acabar se
    /// apunta en `hechos`, un paso por gesto, como antes.
    cambio_en: Option<usize>,
    ultimo_trazo: u64,
    arrastre: Option<Arrastre>,
    lateral: Option<(f32, u64)>,
    x_meta: Option<f32>,
    pastilla_hasta: u64,
    panel: bool,
    poniendo_marca: bool,
    encima: DestinoRiel,
    aviso: Option<(String, u64)>,
    botones: Vec<(RectF, Accion)>,
    raton: (f32, f32),
    /// Por que no se pudo abrir, si no se pudo.
    error: Option<String>,
    /// Se esta escribiendo el PDF anotado.
    exportando: bool,
    /// Buscar dentro (D9): la caja y el texto de las hojas.
    hallar: HallarPdf,
    /// El nombre de la pastilla y, si se esta cambiando, lo escrito (D5).
    renombre: crate::renombrar_doc::Pastilla,
    /// **Escuchando** (la barra de abajo): la voz y los trozos de las hojas (`voz`).
    voz: Option<voz::VozDelPdf>,
    /// Se pidio escuchar y el texto del PDF aun no habia llegado.
    escuchar_al_llegar: bool,
}

pub fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    ruta: &Path,
) -> Result<()> {
    // Cuenta como abierto para los grupos de ventanas (H9) mientras viva.
    let _grupo = crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lector {
        ruta: ruta.to_path_buf(),
    });
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let area = monitor.area;
    let marco = Marco {
        ancho: area.ancho as f32,
        alto: area.alto as f32,
        e: monitor.escala_por_cien as f32 / 100.0,
        escala_por_cien: monitor.escala_por_cien,
        area,
    };
    let ventana = VentanaOverlay::nueva_normal(area, &textos.t("lector-pdf-titulo"))
        .context("no se pudo abrir la ventana del lector de PDF")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        area.ancho,
        area.alto,
    )
    .context("sin superficie para el lector de PDF")?;
    ventana.mostrar();
    ventana.enfocar();
    // Todos los puntos del trazo y la presion del lapiz, como en el lienzo.
    ventana.pedir_entrada_fina();

    let (tx_pedidos, rx_pedidos) = mpsc::channel::<Pedido>();
    let (tx_llega, rx_llega) = mpsc::channel::<Llega>();
    let hwnd = ventana.handle().0 as isize;
    // Antes que nada: de que proyecto es, y lo anotado antes junto al PDF
    // adoptado en sus hojas. Se pinta su copia limpia si la tiene.
    let donde = crate::lector_pdf_proyecto::DondeVa::de(ubicacion.raiz(), ruta, ahora_ms() as i64);
    let ruta_hilo = donde.documento(ruta);
    let hilo = std::thread::Builder::new()
        .name("lector-pdf-hojas".into())
        .spawn(move || hilo_de_hojas(ruta_hilo, rx_pedidos, tx_llega, hwnd))
        .context("sin hilo para dibujar las hojas")?;

    // Con lo que viaja del chat encima, si es un adjunto (v0.96).
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let mut e = Estado {
        nombre: pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta)),
        marcas: marcas::de_texto(&ajustes.marcas),
        zoom: ajustes.zoom,
        ajustes,
        medidas: Vec::new(),
        hojas: vista::Hojas::default(),
        pintadas: HashMap::new(),
        ultima_lista: Vec::new(),
        x: 0.0,
        y: 0.0,
        zoom_cambiado: 0,
        capas: HashMap::new(),
        donde,
        hechos: Vec::new(),
        deshechos: Vec::new(),
        anotando: false,
        tinta: Tinta::nueva(),
        cambio_en: None,
        ultimo_trazo: 0,
        arrastre: None,
        lateral: None,
        x_meta: None,
        pastilla_hasta: ahora_ms() + MS_DE_LA_PASTILLA,
        panel: false,
        poniendo_marca: false,
        encima: DestinoRiel::Fuera,
        aviso: None,
        botones: Vec::new(),
        raton: (0.0, 0.0),
        error: None,
        exportando: false,
        hallar: HallarPdf::default(),
        renombre: crate::renombrar_doc::Pastilla::de(ubicacion.raiz(), ruta),
        voz: None,
        escuchar_al_llegar: false,
    };

    // El nombre de la pastilla: el del mensaje del chat si es de uno.
    e.nombre = pixpin_docs::sin_extension(&e.renombre.visto);
    if recoger_el_indice(&mut e, ruta) {
        guardar_ajustes(&mut e, ruta);
    }
    let mut hay_que_pintar = true;
    let mut vivo = true;
    // La ruta cambia si se renombra un PDF suelto (D5).
    let mut ruta_viva = ruta.to_path_buf();
    let mut ruta_nueva: Option<PathBuf> = None;
    while vivo {
        let ruta_de_la_vuelta = ruta_viva.clone();
        let ruta: &Path = &ruta_de_la_vuelta;
        pixpin_shell::overlay::bombear_pendientes();
        // Lo que manda el hilo de las hojas.
        // El texto del PDF, cuando llega de su hilo (D9).
        if recoger_texto(&mut e, marco) {
            hay_que_pintar = true;
        }
        // La voz: lo siguiente que decir (y empezar si llego el texto).
        if voz::vuelta(&mut e, textos, ruta, marco) {
            hay_que_pintar = true;
        }
        while let Ok(l) = rx_llega.try_recv() {
            hay_que_pintar = true;
            let son_las_hojas = matches!(l, Llega::Medidas(_));
            recibir(&mut e, l, &motor, textos);
            if son_las_hojas {
                al_saber_las_hojas(&e, ruta);
            }
        }
        for (h, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
            // **Anotando, la tinta primero**: las herramientas del lienzo
            // (`lector_tinta::Tinta`), cada evento a la capa de su hoja. El
            // clic mira antes el riel y los botones del lector; Escape solo es
            // de la tinta si tiene algo que soltar; y Ctrl+Z/Ctrl+Y van al
            // deshacer del lector, que sabe en que hoja fue lo ultimo.
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
                    EventoOverlay::Tecla {
                        vk: VK_Z | VK_Y,
                        ctrl: true,
                        ..
                    } => e.tinta.gesto.esta_escribiendo(),
                    // Los corchetes con Ctrl son los margenes del PDF, salvo con
                    // algo elegido, que entonces lo suben o lo bajan.
                    EventoOverlay::Tecla {
                        vk: VK_CORCHETE_ABRE | VK_CORCHETE_CIERRA,
                        ctrl: true,
                        ..
                    } => !e.tinta.gesto.seleccion.esta_vacia(),
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
                    if let EventoOverlay::RatonMovido(p) | EventoOverlay::BotonPulsado(p) = evento {
                        e.raton = local(p);
                    }
                    let h = a_la_tinta(&mut e, &evento, ruta, marco);
                    if let Some(c) = h.cursor {
                        ventana.poner_cursor(crate::dibujo::teclas::forma_de(c));
                    }
                    if h.cambio {
                        e.ultimo_trazo = ahora_ms();
                        e.cambio_en = e.tinta.hoja;
                    }
                    if h.arrastra {
                        e.arrastre = Some(Arrastre::Trazo);
                        ventana.capturar_raton();
                    }
                    let acaba = match evento {
                        EventoOverlay::BotonSoltado(_) => {
                            if matches!(e.arrastre, Some(Arrastre::Trazo)) {
                                e.arrastre = None;
                                ventana.soltar_raton();
                            }
                            true
                        }
                        EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_) => false,
                        _ => !e.tinta.trazando(),
                    };
                    // Un paso de deshacer por gesto: el lector deshace en la
                    // hoja de lo ultimo que se hizo.
                    if acaba && let Some(i) = e.cambio_en.take() {
                        e.hechos.push(i);
                        e.deshechos.clear();
                    }
                    if h.salir {
                        alternar_anotar(&mut e, ruta);
                    }
                    if let EventoOverlay::RatonMovido(_) = evento {
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
                    if pulsar(&mut e, textos, ruta, ubicacion, &tx_pedidos, marco) {
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
                    lado(&mut e, lector::muescas(delta) * 60.0, marco)
                }
                // Ctrl+Mayus+S: compartir, el atajo del editor. Aqui y no en
                // `tecla`, que no tiene donde viven los datos.
                EventoOverlay::Tecla {
                    vk: VK_S,
                    shift: true,
                    ctrl: true,
                    ..
                } => hacer(
                    &mut e,
                    Accion::Compartir,
                    textos,
                    ruta,
                    ubicacion,
                    &tx_pedidos,
                    marco,
                ),
                // L: escuchar (play/pausa con la barra abierta).
                EventoOverlay::Tecla {
                    vk: VK_L,
                    ctrl: false,
                    ..
                } if !e.anotando && !escribiendo(&e) => voz::escuchar(&mut e, textos, ruta, marco),
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } => {
                    let (suya, nueva) =
                        tecla_de_las_cajas(&mut e, vk, shift, ctrl, textos, ruta, ubicacion);
                    if nueva.is_some() {
                        ruta_nueva = nueva;
                    }
                    if !suya && !tecla(&mut e, vk, shift, ctrl, ruta, marco) {
                        vivo = false;
                    }
                }
                // Llevar a la guarda una llamada que cambia `e` la esconderia.
                #[allow(clippy::collapsible_match)]
                EventoOverlay::Caracter(c) => {
                    if !caracter_de_las_cajas(&mut e, c, ruta) {
                        caracter(&mut e, c, ruta);
                    }
                }
                _ => {}
            }
        }
        if let Some(nueva) = ruta_nueva.take() {
            e.nombre = pixpin_docs::sin_extension(&e.renombre.visto);
            ruta_viva = nueva;
        }
        if !vivo {
            break;
        }

        let ahora = ahora_ms();
        if e.pastilla_hasta >= ahora && e.pastilla_hasta < ahora + 200 {
            hay_que_pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            hay_que_pintar = true;
        }
        if let Some(meta) = e.x_meta {
            let falta = meta - e.x;
            if falta.abs() * px(&e, marco) < 0.5 {
                e.x = meta;
                e.x_meta = None;
            } else {
                e.x += falta * 0.25;
            }
            hay_que_pintar = true;
        }
        if e.arrastre.is_none() && e.lateral.is_some_and(|(_, cuando)| ahora > cuando + 250) {
            fin_de_lateral(&mut e, marco);
            hay_que_pintar = true;
        }
        // El aumento acaba de quedarse quieto: toca pedir las hojas nitidas.
        if e.zoom_cambiado > 0 && ahora > e.zoom_cambiado + MS_ZOOM_FIRME {
            e.zoom_cambiado = 0;
            hay_que_pintar = true;
        }
        // El gesto de pararse: el trazo quieto se convierte en figura.
        let s = px(&e, marco);
        if e.anotando
            && let Some(i) = e.tinta.hoja
            && let Some(capa) = e.capas.get_mut(&i)
            && e.tinta.forma_rapida(capa, s)
        {
            e.ultimo_trazo = ahora;
            hay_que_pintar = true;
        }
        let sucias = e.capas.values().any(|c| c.sucia) && !e.tinta.trazando();
        if sucias && ahora > e.ultimo_trazo + MS_PARA_GUARDAR {
            guardar_capas(&mut e, ruta);
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            acotar(&mut e, marco);
            pedir_hojas(&mut e, &tx_pedidos, marco);
            cargar_capas_visibles(&mut e, ruta, marco);
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| pintar(&mut e, p, marco, textos));
                let _ = superficie.presentar();
            }
        }
        let tope = if e.x_meta.is_some() { 16 } else { 50 };
        let tope = e.tinta.mano.tope_forma_ms().map_or(tope, |t| t.min(tope));
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }

    let ruta: &Path = &ruta_viva;
    // La voz se calla con el lector (el verde ya esta en las marcas).
    e.voz = None;
    guardar_capas(&mut e, ruta);
    if !e.hojas.arriba.is_empty() {
        e.ajustes.pagina = e.hojas.sitio(e.y);
    }
    e.ajustes.zoom = e.zoom;
    guardar_ajustes(&mut e, ruta);
    // Soltar el canal despierta al hilo, que ve que no hay nadie y acaba.
    drop(tx_pedidos);
    if e.exportando {
        // Un PDF anotado a medio escribir no se deja tirado: se espera.
        let _ = hilo.join();
    }
    Ok(())
}

fn recibir(e: &mut Estado, l: Llega, motor: &pixpin_render::MotorRender, textos: &Catalogo) {
    match l {
        Llega::Medidas(medidas) => {
            e.medidas = medidas;
            e.hojas = vista::Hojas::colocar(&e.medidas);
            // Se vuelve a la hoja y al punto de ella donde se dejo, con la
            // marca de sitio arriba, un poco por debajo del borde.
            e.y = e.hojas.y_de(e.ajustes.pagina);
        }
        Llega::Hoja(i, img) => {
            // Una hoja girada llega con otra proporcion que la de su papel
            // (ver `pixpin_pdf::Documento::medidas`): se corrige su sitio sin
            // perder por donde se iba.
            if let Some(medida) = e.medidas.get_mut(i) {
                let antes = if medida.0 > 0.0 {
                    medida.1 / medida.0
                } else {
                    0.0
                };
                let ahora = img.alto as f32 / img.ancho.max(1) as f32;
                if (antes - ahora).abs() > 0.02 {
                    let sitio = e.hojas.sitio(e.y);
                    *medida = (img.ancho as f32, img.alto as f32);
                    e.hojas = vista::Hojas::colocar(&e.medidas);
                    e.y = e.hojas.y_de(sitio);
                }
            }
            match motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles) {
                Ok(b) => {
                    e.pintadas.insert(i, (img.ancho, b));
                }
                Err(err) => tracing::warn!(?err, hoja = i, "no se pudo subir la hoja"),
            }
        }
        Llega::NoSeAbre(texto) => e.error = Some(texto),
        Llega::Progreso(n, total) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("hechas", n);
            args.set("total", total);
            e.aviso = Some((
                textos.t_args("lector-exportando", &args),
                ahora_ms() + 60_000,
            ));
        }
        Llega::Exportado(hecho) => {
            e.exportando = false;
            let texto = match hecho {
                Ok(destino) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("nombre", pixpin_docs::nombre(&destino));
                    let _ = pixpin_shell::abrir_ubicacion(&destino);
                    textos.t_args("visor-guardado", &args)
                }
                Err(err) => {
                    tracing::warn!(%err, "no se pudo escribir el PDF anotado");
                    textos.t("visor-no-guardado")
                }
            };
            e.aviso = Some((texto, ahora_ms() + 4000));
        }
    }
}

// ---------------------------------------------------------------------------
// La vista

/// Lo que mide una hoja en pantalla a aumento 1: una hoja comoda de leer,
/// no de borde a borde de un monitor apaisado (saldria una A4 de dos metros).
fn ancho_base(m: Marco) -> f32 {
    (m.ancho - 120.0 * m.e).min(1000.0 * m.e).max(300.0 * m.e)
}

/// Pixeles de pantalla por unidad.
fn px(e: &Estado, m: Marco) -> f32 {
    ancho_base(m) / vista::ANCHO_HOJA * e.zoom
}

fn limites(e: &Estado) -> (f32, f32) {
    let (izq, der) = vista::espacios_del_pdf(e.ajustes.espacios);
    (-izq, vista::ANCHO_HOJA + der)
}

fn zoom_minimo(e: &Estado, m: Marco) -> f32 {
    let (izq, der) = limites(e);
    let cabe = vista::zoom_minimo(der - izq, m.ancho, ancho_base(m) / vista::ANCHO_HOJA);
    cabe.min(ZOOM_MINIMO_PDF)
}

fn acotar(e: &mut Estado, m: Marco) {
    e.zoom = e.zoom.clamp(zoom_minimo(e, m), vista::ZOOM_MAXIMO_PDF);
    let s = px(e, m);
    let (izq, der) = limites(e);
    let (x, y) = vista::dentro(
        e.x,
        e.y + AIRE,
        izq,
        der,
        e.hojas.total + 2.0 * AIRE,
        m.ancho / s,
        m.alto / s,
    );
    e.x = x;
    e.y = y - AIRE;
}

/// El ancho al que conviene rasterizar la hoja `i` con el aumento de ahora.
fn ancho_para(e: &Estado, i: usize, m: Marco) -> u32 {
    let deseado = (vista::ANCHO_HOJA * px(e, m)).ceil().max(1.0) as u32;
    let escalonado = deseado.div_ceil(ESCALON) * ESCALON;
    let proporcion = e
        .hojas
        .altos
        .get(i)
        .map_or(1.414, |a| a / vista::ANCHO_HOJA)
        .max(0.05);
    let cabe = (PIXELES_POR_HOJA / proporcion).sqrt() as u32;
    escalonado.min(cabe).clamp(ESCALON, 8192)
}

/// Las hojas que se ven.
fn visibles(e: &Estado, m: Marco) -> std::ops::Range<usize> {
    let s = px(e, m);
    e.hojas.visibles(e.y, e.y + m.alto / s)
}

/// Pide al hilo las hojas que faltan o que se ven borrosas, las que se ven
/// primero. Tambien suelta las que ya quedan lejos.
fn pedir_hojas(e: &mut Estado, tx: &mpsc::Sender<Pedido>, m: Marco) {
    if e.hojas.cuantas() == 0 {
        return;
    }
    let vis = visibles(e, m);
    let reserva = vis.start.saturating_sub(1)..(vis.end + 1).min(e.hojas.cuantas());
    let firme = e.zoom_cambiado == 0;
    let mut lista = Vec::new();
    for i in vis
        .clone()
        .chain(reserva.clone().filter(|i| !vis.contains(i)))
    {
        let quiero = ancho_para(e, i, m);
        match e.pintadas.get(&i) {
            // Mientras la rueda cambia el aumento se estira lo que hay.
            Some((ancho, _)) if *ancho == quiero || !firme => {}
            _ => lista.push((i, quiero)),
        }
    }
    if lista != e.ultima_lista {
        let _ = tx.send(Pedido::Quiero(lista.clone()));
        e.ultima_lista = lista;
    }
    let lejos = vis.start.saturating_sub(HOJAS_DE_RESERVA)..vis.end + HOJAS_DE_RESERVA;
    e.pintadas.retain(|i, _| lejos.contains(i));
}

/// Lee del disco lo anotado de las hojas que se ven (una vez cada una).
fn cargar_capas_visibles(e: &mut Estado, ruta: &Path, m: Marco) {
    for i in visibles(e, m) {
        let (donde, espacios, alto) = (&e.donde, e.ajustes.espacios, alto_de(&e.hojas, i));
        e.capas
            .entry(i)
            .or_insert_with(|| donde.leer_capa(ruta, i, espacios, alto));
    }
}

/// La camara de la hoja `i`: la misma transformada con la que `pintar` pone
/// su vista (cada hoja con su propio cero), para que la tinta traduzca el
/// raton exactamente donde se pinta.
fn camara_de_la_hoja(e: &Estado, i: usize, m: Marco) -> pixpin_motor2d::camara::Camara {
    pixpin_motor2d::camara::Camara {
        x: e.x,
        y: e.y - e.hojas.arriba.get(i).copied().unwrap_or(0.0),
        zoom: px(e, m),
    }
}

/// Un evento para la tinta, con la capa de su hoja: la del gesto en curso,
/// o al pulsar, la de debajo del raton (lo elegido en otra hoja se suelta).
fn a_la_tinta(e: &mut Estado, ev: &EventoOverlay, ruta: &Path, m: Marco) -> lector_tinta::Hecho {
    let bajo = en_la_hoja(e, m).map(|(i, _)| i);
    if matches!(ev, EventoOverlay::BotonPulsado(_))
        && let Some(i) = bajo
    {
        e.tinta.a_la_hoja(i);
    }
    let Some(i) = e.tinta.hoja.or(bajo) else {
        return lector_tinta::Hecho::default();
    };
    e.tinta.hoja = Some(i);
    let camara = camara_de_la_hoja(e, i, m);
    let (donde, espacios, alto) = (&e.donde, e.ajustes.espacios, alto_de(&e.hojas, i));
    let capa = e
        .capas
        .entry(i)
        .or_insert_with(|| donde.leer_capa(ruta, i, espacios, alto));
    e.tinta.evento(ev, capa, &camara, m.area, m.escala_por_cien)
}

/// Si el clic es del lector aunque se este anotando: el riel, sus botones,
/// o cerrar lo que hubiera abierto.
fn el_lector_primero(e: &Estado, m: Marco) -> bool {
    !matches!(
        riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1)),
        DestinoRiel::Fuera
    ) || que_hay_debajo(e).is_some()
        || e.panel
        || e.poniendo_marca
}

/// La hoja bajo el raton y el punto de ella, en sus unidades.
fn en_la_hoja(e: &Estado, m: Marco) -> Option<(usize, Punto2)> {
    if e.hojas.cuantas() == 0 {
        return None;
    }
    let s = px(e, m);
    let y = e.y + e.raton.1 / s;
    let i = e.hojas.en(y);
    Some((i, Punto2::nuevo(e.x + e.raton.0 / s, y - e.hojas.arriba[i])))
}

/// Por donde va la lectura: la hoja de arriba y la fraccion que ha pasado.
fn sitio(e: &Estado) -> f64 {
    e.hojas.sitio(e.y.max(0.0))
}

// ---------------------------------------------------------------------------
// Pintar

fn pintar(e: &mut Estado, p: &Pintor, m: Marco, textos: &Catalogo) {
    let mut botones = Vec::new();
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: m.ancho,
            alto: m.alto,
        },
        FONDO_PDF,
    );
    if let Some(err) = &e.error {
        let texto = format!("{} — {err}", textos.t("lector-pdf-no-se-abre"));
        let (w, _) = p.medir_texto(&texto, 15.0 * m.e);
        p.texto(&texto, (m.ancho - w) / 2.0, m.alto / 2.0, 15.0 * m.e, TEXTO);
        e.botones = botones;
        return;
    }
    if e.hojas.cuantas() == 0 {
        let texto = textos.t("lector-pdf-abriendo");
        let (w, _) = p.medir_texto(&texto, 15.0 * m.e);
        p.texto(
            &texto,
            (m.ancho - w) / 2.0,
            m.alto / 2.0,
            15.0 * m.e,
            APAGADO,
        );
        e.botones = botones;
        return;
    }

    let s = px(e, m);
    let (izq, der) = vista::espacios_del_pdf(e.ajustes.espacios);
    let vis = visibles(e, m);
    for i in vis {
        let arriba = e.hojas.arriba[i];
        let alto = e.hojas.altos[i];
        // Cada hoja con su propio cero: la hoja y su tinta, en la misma
        // transformada.
        p.poner_vista((0.0, 0.0), s, (-e.x * s, (arriba - e.y) * s));
        if izq > 0.0 {
            p.rellenar(
                RectF {
                    x: -izq,
                    y: 0.0,
                    ancho: izq,
                    alto,
                },
                PAPEL_DEL_MARGEN,
            );
        }
        if der > 0.0 {
            p.rellenar(
                RectF {
                    x: vista::ANCHO_HOJA,
                    y: 0.0,
                    ancho: der,
                    alto,
                },
                PAPEL_DEL_MARGEN,
            );
        }
        let hoja = RectF {
            x: 0.0,
            y: 0.0,
            ancho: vista::ANCHO_HOJA,
            alto,
        };
        match e.pintadas.get(&i) {
            Some((_, b)) => p.bitmap(b, hoja, None, false),
            None => {
                p.rellenar(hoja, Color::BLANCO);
                let n = format!("{}", i + 1);
                p.texto(&n, 40.0, 40.0, 48.0, con_alfa(APAGADO, 0.6));
            }
        }
        if izq > 0.0 || der > 0.0 {
            // El canto de la hoja contra el margen: se ve donde acaba el
            // papel del documento y empieza el de anotar.
            let linea = 1.0 / s;
            for x in [0.0, vista::ANCHO_HOJA - linea] {
                p.rellenar(
                    RectF {
                        x,
                        y: 0.0,
                        ancho: linea,
                        alto,
                    },
                    con_alfa(RAYA, 0.5),
                );
            }
        }
        marcas_de_busqueda(e, p, i);
        voz::pintar_lo_que_suena(e, p, i);
        if let Some(capa) = e.capas.get(&i) {
            let vista_hoja = (
                e.x,
                e.y - arriba,
                e.x + m.ancho / s,
                e.y - arriba + m.alto / s,
            );
            // En la misma pasada y con la misma transformada que la hoja.
            let activa = e.anotando && e.tinta.hoja == Some(i);
            e.tinta.pintar_capa(
                p,
                i,
                capa,
                vista_hoja,
                s,
                lector_tinta::papel(Color::BLANCO),
                activa,
            );
        }
    }
    p.desplazar(0.0, 0.0);

    // Las marcas, en su sitio del documento: un redondel en el canto
    // derecho de su hoja, a su altura.
    for mm in &e.marcas {
        let i = marcas::pagina_de(mm) as usize;
        if i >= e.hojas.cuantas() {
            continue;
        }
        let y_doc = e.hojas.arriba[i] + marcas::alto_en_la_pagina(mm) as f32 * e.hojas.altos[i];
        let x_doc = vista::ANCHO_HOJA + 20.0;
        let (sx, sy) = ((x_doc - e.x) * s, (y_doc - e.y) * s);
        if sy > -40.0 && sy < m.alto + 40.0 {
            lector::redondel(
                p,
                &mm.emoji,
                (sx.min(m.ancho - 90.0 * m.e), sy),
                14.0 * m.e,
                0.8,
            );
        }
    }

    barra_de_avance(e, p, m);
    let emojis: Vec<&str> = e.marcas.iter().map(|x| x.emoji.as_str()).collect();
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
    // Abajo: la barra de escuchar o los mandos de los lados (los mismos del
    // lector de Word: espacio a cada lado y el candado), con la pastilla o
    // con el raton en la franja de abajo.
    if e.voz.is_some() && !e.poniendo_marca {
        for (r, b) in voz::pintar_barra(e, p, m) {
            botones.push((r, Accion::Voz(b)));
        }
    } else if !e.anotando
        && !e.panel
        && !e.poniendo_marca
        && e.hojas.cuantas() > 0
        // Solo con el raton abajo: al abrir no salen solos (queja del usuario:
        // el «+» solo donde se dibuja; el movil los ensena al tocar).
        && lector::raton_abajo(e.raton.1, m.alto, m.e)
    {
        let izq = u32::from(e.ajustes.espacios & vista::ESPACIO_IZQUIERDA != 0);
        let der = u32::from(e.ajustes.espacios & vista::ESPACIO_DERECHA != 0);
        for (r, b) in lector::pintar_mandos_de_los_lados(
            p,
            m.ancho,
            m.alto,
            m.e,
            izq,
            der,
            1,
            e.ajustes.sin_lado,
        ) {
            botones.push((r, Accion::Lado(b)));
        }
    }
    if e.anotando {
        // La barra y el panel del lienzo (`lector_tinta::Tinta`).
        let activa = e.tinta.hoja.and_then(|i| e.capas.get(&i));
        e.tinta
            .pintar_interfaz(p, activa, m.area, m.escala_por_cien);
    } else if e.renombre.editando.is_some()
        || (!e.hallar.caja.abierto && (e.pastilla_hasta > ahora_ms() || e.panel))
    {
        pastilla(e, p, m, &mut botones);
    }
    if e.renombre.editando.is_none() {
        let aviso = aviso_de_buscar(e, textos);
        for (zona, b) in e
            .hallar
            .caja
            .pintar(p, m.ancho, m.e, textos, aviso.as_deref())
        {
            botones.push((zona, Accion::Buscar(b)));
        }
    }
    if e.panel {
        panel(e, p, m, textos, &mut botones);
    }
    if let Some((texto, _)) = &e.aviso {
        lector::aviso_abajo(p, texto, m.ancho, m.alto, m.e);
    }
    e.botones = botones;
}

fn barra_de_avance(e: &Estado, p: &Pintor, m: Marco) {
    let s = px(e, m);
    let visto = m.alto / s;
    if e.hojas.total <= visto {
        return;
    }
    let ancho = 3.0 * m.e;
    let largo = (m.alto * visto / e.hojas.total).max(30.0 * m.e);
    let y = (e.y / (e.hojas.total - visto)).clamp(0.0, 1.0) * (m.alto - largo);
    p.rellenar_redondeado(
        RectF {
            x: m.ancho - ancho - 4.0 * m.e,
            y,
            ancho,
            alto: largo,
        },
        ancho / 2.0,
        RAYA,
    );
}

fn pastilla(e: &Estado, p: &Pintor, m: Marco, botones: &mut Vec<(RectF, Accion)>) {
    let tam = 15.0 * m.e;
    // Escribiendo el nombre (D5) se ensena solo lo escrito, sin la hoja.
    let hoja = if let Some(t) = &e.renombre.editando {
        if t.is_empty() {
            " ".to_string()
        } else {
            t.clone()
        }
    } else if e.hojas.cuantas() > 0 {
        format!(
            "{} · {}/{}",
            e.nombre,
            e.hojas.en(e.y.max(0.0)) + 1,
            e.hojas.cuantas()
        )
    } else {
        e.nombre.clone()
    };
    let (w, h) = p.medir_texto(&hoja, tam);
    let lado = 34.0 * m.e;
    let total = w + 28.0 * m.e + 2.0 * lado;
    let caja = RectF {
        x: (m.ancho - total) / 2.0,
        y: 14.0 * m.e,
        ancho: total,
        alto: h + 14.0 * m.e,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(CRISTAL, 0.94));
    p.texto(&hoja, caja.x + 14.0 * m.e, caja.y + 7.0 * m.e, tam, TEXTO);
    if let Some(t) = &e.renombre.editando {
        // Escribiendo el nombre: subrayado y cursor detras de lo escrito.
        let (wt, _) = p.medir_texto(t, tam);
        let x0 = caja.x + 14.0 * m.e;
        p.rellenar(
            RectF {
                x: x0,
                y: caja.y + caja.alto - 6.0 * m.e,
                ancho: wt.max(40.0 * m.e),
                alto: 1.0 * m.e,
            },
            DORADO,
        );
        p.rellenar(
            RectF {
                x: x0 + wt + 1.0,
                y: caja.y + 8.0 * m.e,
                ancho: 1.5 * m.e,
                alto: h - 2.0 * m.e,
            },
            DORADO,
        );
    }
    if e.renombre.se_puede() {
        botones.push((
            RectF {
                x: caja.x,
                y: caja.y,
                ancho: w + 22.0 * m.e,
                alto: caja.alto,
            },
            Accion::Renombrar,
        ));
    }
    // **Escuchar** (`RecordVoiceOver`) y el engranaje, como la pastilla del movil.
    let x0 = caja.x + 14.0 * m.e + w + 4.0 * m.e;
    let l = 19.0 * m.e;
    for (n, (icono, encendido, que)) in [
        (
            &material::RECORD_VOICE_OVER,
            e.voz.is_some(),
            Accion::Escuchar,
        ),
        (&material::SETTINGS, e.panel, Accion::Engranaje),
    ]
    .into_iter()
    .enumerate()
    {
        let zona = RectF {
            x: x0 + n as f32 * lado,
            y: caja.y,
            ancho: lado,
            alto: caja.alto,
        };
        p.icono(
            icono,
            RectF {
                x: zona.x + (lado - l) / 2.0,
                y: zona.y + (zona.alto - l) / 2.0,
                ancho: l,
                alto: l,
            },
            if encendido { DORADO } else { APAGADO },
        );
        botones.push((zona, que));
    }
}

fn filas_del_panel(e: &Estado, textos: &Catalogo) -> Vec<(String, Accion)> {
    let mut filas = vec![
        (textos.t("lector-escuchar"), Accion::Escuchar),
        (textos.t("compartir-lector"), Accion::Compartir),
        (textos.t("lector-exportar-pdf"), Accion::Exportar),
        (textos.t("visor-abrir-carpeta"), Accion::AbrirCarpeta),
    ];
    if !e.marcas.is_empty() {
        filas.push((textos.t("visor-quitar-marcadores"), Accion::QuitarMarcas));
    }
    filas.push((textos.t("lector-quitar-tinta"), Accion::QuitarTinta));
    // Solo si no lo es ya: como en el movil, anotar no crea ningun proyecto.
    if !e.donde.es_de_un_proyecto() {
        filas.push((textos.t("lector-al-proyecto"), Accion::AlProyecto));
    }
    filas
}

fn panel(e: &Estado, p: &Pintor, m: Marco, textos: &Catalogo, botones: &mut Vec<(RectF, Accion)>) {
    let filas = filas_del_panel(e, textos);
    let alto_panel = (60.0 + filas.len() as f32 * 30.0) * m.e;
    let caja = RectF {
        x: 0.0,
        y: m.alto - alto_panel,
        ancho: m.ancho,
        alto: alto_panel,
    };
    p.rellenar(caja, CRISTAL);
    botones.push((caja, Accion::Engranaje));
    let x = 90.0 * m.e;
    for (i, (etiqueta, que)) in filas.into_iter().enumerate() {
        let y = caja.y + (16.0 + i as f32 * 30.0) * m.e;
        p.texto(&etiqueta, x, y, 14.0 * m.e, TEXTO);
        botones.push((
            RectF {
                x,
                y: y - 4.0 * m.e,
                ancho: 380.0 * m.e,
                alto: 28.0 * m.e,
            },
            que,
        ));
    }
    p.texto(
        &textos.t("lector-ayuda-pdf"),
        x,
        m.alto - 22.0 * m.e,
        12.0 * m.e,
        APAGADO,
    );
}

// ---------------------------------------------------------------------------
// Raton y teclado

fn que_hay_debajo(e: &Estado) -> Option<Accion> {
    let (x, y) = e.raton;
    e.botones
        .iter()
        .rev()
        .find(|(c, _)| dentro(c, x, y))
        .map(|(_, a)| *a)
}

fn riel_de(e: &Estado, m: Marco) -> pixpin_ui::riel_marcas::Riel {
    lector::riel(
        m.ancho,
        m.alto,
        m.escala_por_cien,
        e.marcas.len(),
        e.poniendo_marca,
    )
}

fn mover_raton(e: &mut Estado, m: Marco) {
    e.encima = riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1));
    match e.arrastre {
        Some(Arrastre::Mover {
            desde,
            vista: v,
            movido,
        }) => {
            let (dx, dy) = (e.raton.0 - desde.0, e.raton.1 - desde.1);
            let movido = movido || dx.abs().max(dy.abs()) > lector::UMBRAL_DE_ARRASTRE;
            if movido {
                let s = px(e, m);
                if !e.ajustes.sin_lado {
                    e.x = v.0 - dx / s;
                }
                e.y = v.1 - dy / s;
                e.x_meta = None;
                acotar(e, m);
            }
            e.arrastre = Some(Arrastre::Mover {
                desde,
                vista: v,
                movido,
            });
        }
        // El trazo lo lleva la tinta, en el bucle.
        Some(Arrastre::Trazo) => {}
        None => {}
    }
}

fn pulsar(
    e: &mut Estado,
    textos: &Catalogo,
    ruta: &Path,
    ubicacion: &Ubicacion,
    tx: &mpsc::Sender<Pedido>,
    m: Marco,
) -> bool {
    e.x_meta = None;
    // Un clic fuera del nombre deja de cambiarlo sin guardar (se guarda con
    // Intro, como en el movil).
    if e.renombre.editando.is_some() && que_hay_debajo(e) != Some(Accion::Renombrar) {
        e.renombre.editando = None;
    }
    match riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1)) {
        DestinoRiel::PonerMarca => {
            e.poniendo_marca = !e.poniendo_marca;
            return false;
        }
        DestinoRiel::Hojita => {
            alternar_anotar(e, ruta);
            return false;
        }
        DestinoRiel::Marca(i) => {
            ir_a_la_marca(e, i);
            return false;
        }
        DestinoRiel::Emoji(i) => {
            poner_marca(e, i, ruta);
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
        hacer(e, a, textos, ruta, ubicacion, tx, m);
        return false;
    }
    if e.panel || e.poniendo_marca {
        e.panel = false;
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
            // El trazo lo lleva la tinta; el paso de deshacer lo apunta el
            // bucle al acabar el gesto.
            e.ultimo_trazo = ahora_ms();
        }
        Some(Arrastre::Mover { movido: false, .. }) => {
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
    if let DestinoRiel::Marca(i) = riel_de(e, m).destino(lector::punto(e.raton.0, e.raton.1))
        && let Some(id) = e.marcas.get(i).map(|x| x.id)
    {
        e.marcas = marcas::sin(&e.marcas, id);
        guardar_ajustes(e, ruta);
    }
}

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
    // El centro es la hoja en medio de la ventana.
    let centro = vista::ANCHO_HOJA / 2.0 - vista_ancho / 2.0;
    let margen = vista::ANCHO_HOJA * vista::MARGEN_DEL_PDF;
    if let Some(meta) = vista::iman_del_centro(antes, e.x, centro, margen) {
        e.x_meta = Some(meta);
    }
}

fn rueda(e: &mut Estado, delta: i32, m: Marco) {
    let modificadores = pixpin_shell::entrada::modificadores_pulsados();
    if modificadores.ctrl {
        acercar(e, lector::muescas(delta), m);
    } else if modificadores.shift {
        lado(e, -lector::muescas(delta) * 60.0, m);
    } else {
        // Tres «renglones» de 40 pixeles de pantalla por muesca: pasar
        // hojas no depende del aumento que se tenga.
        e.y += lector::desplazamiento_de_rueda(delta, 40.0 * m.e / px(e, m));
        acotar(e, m);
    }
}

fn lado(e: &mut Estado, cuanto: f32, m: Marco) {
    // Con el candado de los mandos de abajo, a lo ancho no se mueve.
    if e.ajustes.sin_lado {
        return;
    }
    let antes = e.lateral.map_or(e.x, |(a, _)| a);
    e.lateral = Some((antes, ahora_ms()));
    e.x_meta = None;
    e.x += cuanto / px(e, m);
    acotar(e, m);
}

fn acercar(e: &mut Estado, muescas: f32, m: Marco) {
    let antes = px(e, m);
    e.zoom = vista::zoom_con_rueda(e.zoom, muescas, zoom_minimo(e, m), vista::ZOOM_MAXIMO_PDF);
    let despues = px(e, m);
    e.x = vista::con_foco(e.x, e.raton.0, antes, despues);
    e.y = vista::con_foco(e.y, e.raton.1, antes, despues);
    e.x_meta = None;
    e.zoom_cambiado = ahora_ms();
    acotar(e, m);
}

fn tecla(e: &mut Estado, vk: u32, shift: bool, ctrl: bool, ruta: &Path, m: Marco) -> bool {
    let s = px(e, m);
    let linea = 40.0 * m.e / s;
    let pantalla = m.alto / s * 0.9;
    // Anotando, una letra que elige una herramienta es de la herramienta: la
    // M de la mano no pone una marca ni la G abre los ajustes.
    if e.anotando && !ctrl && lector_tinta::letra_de_herramienta(vk) {
        return true;
    }
    match (vk, ctrl) {
        (VK_ESCAPE, _) => {
            if e.poniendo_marca || e.panel {
                e.poniendo_marca = false;
                e.panel = false;
            } else if e.voz.is_some() {
                // La primera Esc cierra la barra de escuchar.
                e.voz = None;
            } else if e.anotando {
                alternar_anotar(e, ruta);
            } else {
                return false;
            }
        }
        (VK_DOWN, false) => e.y += linea,
        (VK_UP, false) => e.y -= linea,
        (VK_NEXT, _) => e.y += pantalla,
        (VK_PRIOR, _) => e.y -= pantalla,
        (VK_ESPACIO, false) => e.y += if shift { -pantalla } else { pantalla },
        (VK_HOME, _) => e.y = -AIRE,
        (VK_END, _) => e.y = e.hojas.total,
        (VK_LEFT, false) => lado(e, -60.0 * m.e, m),
        (VK_RIGHT, false) => lado(e, 60.0 * m.e, m),
        (VK_0, true) => {
            e.raton = (m.ancho / 2.0, m.alto / 2.0);
            let antes = px(e, m);
            e.zoom = 1.0;
            let despues = px(e, m);
            e.y = vista::con_foco(e.y, e.raton.1, antes, despues);
            e.zoom_cambiado = ahora_ms();
        }
        (VK_MAS | VK_MAS_NUM, _) => {
            e.raton = (m.ancho / 2.0, m.alto / 2.0);
            acercar(e, 1.0, m);
        }
        (VK_MENOS | VK_MENOS_NUM, _) => {
            e.raton = (m.ancho / 2.0, m.alto / 2.0);
            acercar(e, -1.0, m);
        }
        (VK_CORCHETE_ABRE, true) => alternar_espacio(e, vista::ESPACIO_IZQUIERDA, ruta),
        (VK_CORCHETE_CIERRA, true) => alternar_espacio(e, vista::ESPACIO_DERECHA, ruta),
        (VK_Z, true) => {
            if shift {
                rehacer(e);
            } else {
                deshacer(e);
            }
        }
        (VK_Y, true) => rehacer(e),
        (VK_S, true) => guardar_capas(e, ruta),
        (VK_M, _) => {
            e.poniendo_marca = !e.poniendo_marca;
            e.panel = false;
        }
        (VK_A, false) => alternar_anotar(e, ruta),
        (VK_G, false) => e.panel = !e.panel,
        (v, true) if (0x31..=0x39).contains(&v) => ir_a_la_marca(e, (v - 0x31) as usize),
        _ => e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA,
    }
    acotar(e, m);
    true
}

fn caracter(e: &mut Estado, c: char, ruta: &Path) {
    if e.poniendo_marca {
        if let Some(i) = lector::emoji_de_cifra(c) {
            poner_marca(e, i, ruta);
        }
    }
    // Anotando, las cifras son de la tinta (el grosor, como en el lienzo) y
    // ya las atendio ella en el bucle.
}

// ---------------------------------------------------------------------------
// Hacer

fn alternar_anotar(e: &mut Estado, ruta: &Path) {
    if e.anotando {
        e.anotando = false;
        guardar_capas(e, ruta);
    } else {
        e.anotando = true;
        e.poniendo_marca = false;
        e.panel = false;
    }
}

/// **Al abrir, el marco a toda la tinta que aun no lo tiene** (Android
/// v0.98.5, `LaunchedEffect(cuantas) { capas.fijarLoViejo(cuantas) }`): en
/// cuanto se sabe cuantas hojas hay y lo que mide cada una, la tinta de antes
/// de v0.98 recibe el marco de los espacios de ahora, sin tocarla. Asi, si
/// luego llegan otros espacios del movil, esa tinta ya no se corre. Solo
/// escribe en un adjunto del chat (`fijar_lo_viejo` no toca lo demas).
fn al_saber_las_hojas(e: &Estado, ruta: &Path) {
    if !e.hojas.altos.is_empty() {
        e.donde
            .fijar_lo_viejo(ruta, e.ajustes.espacios, &e.hojas.altos);
    }
}

fn alternar_espacio(e: &mut Estado, lado: u8, ruta: &Path) {
    // **La tinta ya no se corre al poner un espacio** (Android v0.98.0):
    // antes de cambiarlos, la tinta de un adjunto que aun no tiene marco
    // recibe el de la regla vieja con los espacios de ahora
    // (`fijarLoViejo`), y desde ahi manda el marco. Lo abierto ya esta en
    // las unidades de la hoja y se queda como esta.
    e.donde
        .fijar_lo_viejo(ruta, e.ajustes.espacios, &e.hojas.altos);
    e.ajustes.espacios = vista::con_espacio(e.ajustes.espacios, lado);
    guardar_ajustes(e, ruta);
}

/// Una marca donde se esta leyendo: la hoja de arriba y la fraccion de ella
/// (`dondeEstoy` del movil), centrada a lo ancho.
fn poner_marca(e: &mut Estado, i: usize, ruta: &Path) {
    // La ultima de la tira es el verde: uno solo, «se leera desde aqui».
    if i == lector::VERDE_EN_LA_TIRA {
        voz::verde_aqui(e, ruta);
        return;
    }
    let s = sitio(e);
    let pagina = s.floor() as i32;
    let (x, y) = marcas::en_la_pagina(pagina, s - pagina as f64, 0.5);
    let emoji = EMOJIS.get(i).copied().unwrap_or(EMOJIS[0]);
    e.marcas = marcas::con(&e.marcas, x, y, emoji, ahora_ms() as i64);
    e.poniendo_marca = false;
    guardar_ajustes(e, ruta);
}

/// A esa hoja y a esa altura, con la marca **arriba**: es lo que se fue a
/// ver.
fn ir_a_la_marca(e: &mut Estado, i: usize) {
    if let Some(mm) = e.marcas.get(i) {
        e.y = e.hojas.y_de(mm.y);
    }
}

fn deshacer(e: &mut Estado) {
    while let Some(i) = e.hechos.pop() {
        if e.capas.get_mut(&i).is_some_and(|c| c.deshacer()) {
            e.deshechos.push(i);
            e.ultimo_trazo = ahora_ms();
            return;
        }
    }
}

fn rehacer(e: &mut Estado) {
    while let Some(i) = e.deshechos.pop() {
        if e.capas.get_mut(&i).is_some_and(|c| c.rehacer()) {
            e.hechos.push(i);
            e.ultimo_trazo = ahora_ms();
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)] // la accion puede tocar cualquier cosa del lector
fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    ruta: &Path,
    ubicacion: &Ubicacion,
    tx: &mpsc::Sender<Pedido>,
    m: Marco,
) {
    match a {
        Accion::Renombrar => e.renombre.empezar(),
        Accion::AlProyecto => al_proyecto(e, textos, ruta, ubicacion),
        Accion::Buscar(b) => {
            let h = e.hallar.caja.boton(b);
            hecho_de_buscar(e, h, ruta);
        }
        Accion::Engranaje => {
            let en_la_hoja = e.panel && e.raton.1 > m.alto * 0.5;
            if !en_la_hoja {
                e.panel = !e.panel;
            }
        }
        Accion::Escuchar => voz::escuchar(e, textos, ruta, m),
        Accion::Voz(b) => voz::boton(e, b, ruta, m),
        // Los mandos de abajo: en un PDF cada lado tiene un paso (esta o no).
        Accion::Lado(b) => match b {
            lector::BotonLado::Izquierda(mas) => {
                if mas != (e.ajustes.espacios & vista::ESPACIO_IZQUIERDA != 0) {
                    alternar_espacio(e, vista::ESPACIO_IZQUIERDA, ruta);
                }
            }
            lector::BotonLado::Derecha(mas) => {
                if mas != (e.ajustes.espacios & vista::ESPACIO_DERECHA != 0) {
                    alternar_espacio(e, vista::ESPACIO_DERECHA, ruta);
                }
            }
            lector::BotonLado::Candado => {
                e.ajustes.sin_lado = !e.ajustes.sin_lado;
                let clave = if e.ajustes.sin_lado {
                    "lector-sin-lado"
                } else {
                    "lector-con-lado"
                };
                e.aviso = Some((textos.t(clave), ahora_ms() + 2500));
                guardar_ajustes(e, ruta);
            }
        },
        Accion::Exportar => {
            if e.exportando {
                return;
            }
            guardar_capas(e, ruta);
            let tinta = tinta_de_todas(e, ruta);
            let base = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
            let destino = ubicacion
                .raiz()
                .join("exportado")
                .join(format!("{base} ({}).pdf", textos.t("lector-anotado")));
            let pedido = Pedido::Exportar {
                tinta,
                altos: e.hojas.altos.clone(),
                espacios: vista::espacios_del_pdf(e.ajustes.espacios),
                marcas: e.marcas.clone(),
                destino,
            };
            if tx.send(pedido).is_ok() {
                e.exportando = true;
                e.panel = false;
            }
        }
        // Lo anotado, a disco antes: la hoja lo lee de alli, en su hilo.
        Accion::Compartir => {
            guardar_capas(e, ruta);
            e.panel = false;
            crate::compartir::ventana::abrir(
                crate::compartir::idioma_de(ubicacion),
                ubicacion.clone(),
                crate::compartir::Cosa::Documento(ruta.to_path_buf()),
            );
        }
        Accion::AbrirCarpeta => {
            let _ = pixpin_shell::abrir_ubicacion(&ubicacion.raiz().join("exportado"));
        }
        Accion::QuitarMarcas => {
            e.marcas.clear();
            guardar_ajustes(e, ruta);
        }
        Accion::QuitarTinta => {
            for i in 0..e.hojas.cuantas() {
                let de_aqui = e.donde.para_leer(ruta, i);
                if de_aqui.is_file() || e.capas.contains_key(&i) {
                    let (donde, espacios, alto) =
                        (&e.donde, e.ajustes.espacios, alto_de(&e.hojas, i));
                    let capa = e
                        .capas
                        .entry(i)
                        .or_insert_with(|| donde.leer_capa(ruta, i, espacios, alto));
                    capa.vaciar();
                    e.hechos.push(i);
                }
            }
            guardar_capas(e, ruta);
            e.aviso = Some((textos.t("lector-tinta-quitada"), ahora_ms() + 4000));
        }
    }
}

/// **«Al proyecto»** (`LectorPdfActivity.alProyecto`): lo anotado a disco,
/// el PDF pasa a ser un proyecto (`capas_del_pdf::al_proyecto`) y desde ese
/// momento la tinta de cada hoja es la de su hoja del proyecto, la que viaja
/// al sincronizar.
fn al_proyecto(e: &mut Estado, textos: &Catalogo, ruta: &Path, ubicacion: &Ubicacion) {
    guardar_capas(e, ruta);
    let paginas = u32::try_from(e.hojas.cuantas()).unwrap_or(0);
    // Lo anotado hasta ahora, de donde este (con el codigo del mensaje si
    // es un adjunto del chat), pasa a las hojas del proyecto nuevo.
    let donde = e.donde.clone();
    let hecho = pixpin_proyecto::capas_del_pdf::al_proyecto(
        ubicacion.raiz(),
        ruta,
        &e.nombre,
        paginas,
        ahora_ms() as i64,
        &|i| donde.para_leer(ruta, i as usize),
    );
    let aviso = match hecho {
        Ok(ficha) => {
            tracing::info!(%ficha, "PDF del lector pasado a proyecto");
            // Lo abierto se vuelve a leer de las hojas del proyecto.
            e.donde =
                crate::lector_pdf_proyecto::DondeVa::de(ubicacion.raiz(), ruta, ahora_ms() as i64);
            e.capas.clear();
            e.hechos.clear();
            e.deshechos.clear();
            e.tinta.olvidar_lo_calculado();
            e.panel = false;
            crate::ventana_chat::refrescar();
            textos.t("lector-al-proyecto-hecho")
        }
        Err(err) => {
            tracing::warn!(?err, "no se pudo pasar el PDF a proyecto");
            textos.t("lector-al-proyecto-fallo")
        }
    };
    e.aviso = Some((aviso, ahora_ms() + 4000));
}

/// Lo anotado en cada hoja que tiene algo, como ordenes del motor: lo que
/// ya esta abierto y lo que solo esta en disco.
fn tinta_de_todas(e: &mut Estado, ruta: &Path) -> HashMap<usize, Vec<Orden>> {
    let mut salida = HashMap::new();
    for i in 0..e.hojas.cuantas() {
        let ordenes = match e.capas.get(&i) {
            Some(c) => pintado::ordenes_de_escena(&c.escena),
            None => {
                let r = e.donde.para_leer(ruta, i);
                if !r.is_file() {
                    continue;
                }
                pintado::ordenes_de_escena(
                    &e.donde
                        .leer_capa(ruta, i, e.ajustes.espacios, alto_de(&e.hojas, i))
                        .escena,
                )
            }
        };
        if !ordenes.is_empty() {
            salida.insert(i, ordenes);
        }
    }
    salida
}

/// **Los marcadores que ya trae el PDF** (Android v0.98.2): los de su
/// indice (`/Outlines`) —un PDF exportado desde PixPin los lleva ahi— pasan
/// al riel **la primera vez que se abre**, y solo si aun no se guardo
/// ninguno: quitados a mano, no vuelven. «⭐ Hoja 3» vuelve a ser una
/// estrella; lo que no empiece por uno de los nuestros, un 🔖. Devuelve si
/// hay algo que guardar (la marca de que ya se miro, y las marcas).
fn recoger_el_indice(e: &mut Estado, ruta: &Path) -> bool {
    if e.ajustes.indice {
        return false;
    }
    e.ajustes.indice = true;
    // Un adjunto del chat cuyo mensaje ya tiene marcas (las puso el movil,
    // o se quitaron todas: el fichero queda vacio) no las recibe.
    if !e.marcas.is_empty() || crate::anotado_del_adjunto::hay_marcas_del_mensaje(ruta) {
        return true;
    }
    let bytes = std::fs::read(ruta).unwrap_or_default();
    e.marcas = marcas_del_indice(&bytes, ahora_ms() as i64);
    if !e.marcas.is_empty() {
        tracing::info!(
            cuantos = e.marcas.len(),
            "marcadores recogidos del indice del PDF"
        );
    }
    true
}

/// Los marcadores del indice de `bytes` como marcas del lector (ver
/// [`recoger_el_indice`]), puestas a las `ahora`.
fn marcas_del_indice(bytes: &[u8], ahora: i64) -> Vec<Marca> {
    let mut lista = Vec::new();
    for m in pixpin_pdf::con_anotaciones::marcadores_del_indice(bytes, marcas::MAXIMO) {
        let emoji = EMOJIS
            .iter()
            .copied()
            .find(|x| m.titulo.starts_with(x))
            .unwrap_or(EMOJIS[0]);
        let pagina = i32::try_from(m.pagina).unwrap_or(i32::MAX);
        let (x, y) = marcas::en_la_pagina(pagina, m.alto, 0.5);
        lista = marcas::con(&lista, x, y, emoji, ahora);
    }
    lista
}

fn guardar_ajustes(e: &mut Estado, ruta: &Path) {
    e.ajustes.marcas = marcas::a_texto(&e.marcas);
    if let Err(err) = crate::anotado_del_adjunto::escribir(ruta, &e.ajustes) {
        tracing::info!(?err, "no se pudo guardar la lectura junto al PDF");
    }
}

/// El alto de la hoja `i` en unidades del lector (0 si no existe: sin alto
/// no hay marco que leer ni que escribir).
fn alto_de(hojas: &vista::Hojas, i: usize) -> f32 {
    hojas.altos.get(i).copied().unwrap_or(0.0)
}

fn guardar_capas(e: &mut Estado, ruta: &Path) {
    let donde = &e.donde;
    for (i, c) in e.capas.iter_mut() {
        // Solo lo que cambio: pedir donde escribir le pone su dibujo a la
        // hoja del proyecto, y mirar una hoja no la anota.
        if !c.sucia {
            continue;
        }
        // La tinta y, al lado, su marco (`anot-<uid>-p<i>.hoja`).
        if let Err(err) = donde.guardar_capa(ruta, *i, c, alto_de(&e.hojas, *i)) {
            tracing::warn!(?err, hoja = i, "no se pudo guardar lo anotado en la hoja");
        }
    }
}

// ---------------------------------------------------------------------------
// Buscar dentro (D9) y cambiar el nombre (D5)

/// El texto de todas las hojas, o por que no lo hay.
type TextoPdf = Result<Vec<pixpin_pdf::texto::PaginaDeTexto>, pixpin_pdf::texto::SinTexto>;

/// Lo de buscar en el PDF. El texto no se lee al abrir: solo la primera vez
/// que se pulsa Ctrl+F, y en otro hilo (medio segundo en un articulo de
/// veinte hojas medido aqui), para que leer siga siendo inmediato.
#[derive(Default)]
struct HallarPdf {
    caja: crate::buscador::Buscador,
    texto: Option<TextoPdf>,
    leyendo: Option<mpsc::Receiver<TextoPdf>>,
    /// En la siguiente vuelta, llevar la vista a la coincidencia actual.
    saltar: bool,
}

fn escribiendo(e: &Estado) -> bool {
    e.hallar.caja.abierto || e.renombre.editando.is_some()
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
    match e.renombre.tecla(vk, ctrl) {
        Tecla::Consumida => return (true, None),
        Tecla::Confirmar => {
            // Lo pendiente, al nombre de ahora: si el fichero se mueve, se
            // lleva ya su tinta y su sitio.
            guardar_capas(e, ruta);
            guardar_ajustes(e, ruta);
            let hecho = e.renombre.confirmar(ubicacion.raiz(), ruta, textos);
            e.nombre = pixpin_docs::sin_extension(&e.renombre.visto);
            if let Some(t) = hecho.aviso {
                e.aviso = Some((t, ahora_ms() + 3500));
            }
            e.pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
            return (true, hecho.ruta_nueva);
        }
        Tecla::NoEsMia if e.renombre.editando.is_some() => return (false, None),
        Tecla::NoEsMia => {}
    }
    let h = e.hallar.caja.tecla(vk, shift, ctrl);
    if h == crate::buscador::Hecho::NoEsMia {
        return (false, None);
    }
    hecho_de_buscar(e, h, ruta);
    (true, None)
}

fn caracter_de_las_cajas(e: &mut Estado, c: char, ruta: &Path) -> bool {
    if e.renombre.caracter(c) {
        return true;
    }
    match e.hallar.caja.caracter(c) {
        Some(h) => {
            hecho_de_buscar(e, h, ruta);
            true
        }
        None => false,
    }
}

fn hecho_de_buscar(e: &mut Estado, h: crate::buscador::Hecho, ruta: &Path) {
    use crate::buscador::Hecho;
    if matches!(h, Hecho::Abierta | Hecho::Siguiente | Hecho::Anterior) {
        voz::pedir_el_texto(e, ruta);
    }
    match h {
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

/// Busca lo escrito en el texto de las hojas, desde la que se esta leyendo.
fn rebuscar(e: &mut Estado) {
    let Some(Ok(paginas)) = &e.hallar.texto else {
        return;
    };
    let desde = if e.hojas.cuantas() > 0 {
        e.hojas.en(e.y.max(0.0))
    } else {
        0
    };
    e.hallar
        .caja
        .busqueda
        .rehacer(paginas.iter().map(|p| p.texto.as_str()), desde);
    e.hallar.saltar = e.hallar.caja.busqueda.la_actual().is_some();
}

/// Lo que llega del hilo del texto, y el salto pendiente. `true` si hay que
/// repintar.
fn recoger_texto(e: &mut Estado, m: Marco) -> bool {
    let mut cambio = false;
    if let Some(rx) = &e.hallar.leyendo
        && let Ok(t) = rx.try_recv()
    {
        e.hallar.leyendo = None;
        e.hallar.texto = Some(t);
        rebuscar(e);
        cambio = true;
    }
    if e.hallar.saltar {
        saltar_a_la_actual(e, m);
        cambio = true;
    }
    cambio
}

/// Lo que dice la caja en vez de «n de m» mientras no hay que contar.
fn aviso_de_buscar(e: &Estado, textos: &Catalogo) -> Option<String> {
    if !e.hallar.caja.abierto {
        return None;
    }
    if e.hallar.leyendo.is_some() {
        return Some(textos.t("buscar-leyendo"));
    }
    match &e.hallar.texto {
        Some(Err(pixpin_pdf::texto::SinTexto::Cifrado)) => Some(textos.t("buscar-cifrado")),
        Some(Err(_)) => Some(textos.t("buscar-sin-texto")),
        Some(Ok(p)) if p.iter().all(|h| h.texto.trim().is_empty()) => {
            Some(textos.t("buscar-sin-texto"))
        }
        _ => None,
    }
}

/// Las cajas de una coincidencia en unidades de su hoja.
fn cajas_en_la_hoja(e: &Estado, k: pixpin_docs::buscar::Coincidencia) -> Vec<RectF> {
    let (Some(Ok(paginas)), Some(alto)) = (&e.hallar.texto, e.hojas.altos.get(k.texto)) else {
        return Vec::new();
    };
    let Some(p) = paginas.get(k.texto) else {
        return Vec::new();
    };
    p.cajas_de(k.inicio, k.largo)
        .into_iter()
        .map(|c| RectF {
            x: c[0] * vista::ANCHO_HOJA,
            y: c[1] * alto,
            ancho: (c[2] - c[0]) * vista::ANCHO_HOJA,
            alto: (c[3] - c[1]) * alto,
        })
        .collect()
}

/// Marca lo encontrado en la hoja `i`, ya con la transformada de la hoja
/// puesta. Con la caja cerrada no se marca nada.
fn marcas_de_busqueda(e: &Estado, p: &Pintor, i: usize) {
    if !e.hallar.caja.abierto {
        return;
    }
    let b = &e.hallar.caja.busqueda;
    let desde = b.coincidencias.partition_point(|k| k.texto < i);
    for (n, k) in b.coincidencias.iter().enumerate().skip(desde) {
        if k.texto != i {
            break;
        }
        let color = if Some(n) == b.actual {
            crate::buscador::MARCA_ACTUAL
        } else {
            crate::buscador::MARCA
        };
        for r in cajas_en_la_hoja(e, *k) {
            p.rellenar_redondeado(r, 2.0, color);
        }
    }
}

/// Lleva la vista a la coincidencia actual si no se ve (a un tercio de la
/// ventana); sin cajas, al menos a su hoja.
fn saltar_a_la_actual(e: &mut Estado, m: Marco) {
    e.hallar.saltar = false;
    let Some(k) = e.hallar.caja.busqueda.la_actual() else {
        return;
    };
    let Some(arriba) = e.hojas.arriba.get(k.texto).copied() else {
        return;
    };
    let s = px(e, m);
    let (ancho, alto) = (m.ancho / s, m.alto / s);
    let Some(r) = cajas_en_la_hoja(e, k).first().copied() else {
        e.y = arriba;
        return;
    };
    let y = arriba + r.y;
    let tapado = 70.0 * m.e / s;
    if y < e.y + tapado || y + r.alto > e.y + alto {
        e.y = y - alto / 3.0;
    }
    if r.x < e.x || r.x + r.ancho > e.x + ancho {
        e.x = r.x - ancho / 3.0;
        e.x_meta = None;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn marco() -> Marco {
        Marco {
            ancho: 1920.0,
            alto: 1080.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080,
            },
        }
    }

    fn estado(paginas: usize) -> Estado {
        let medidas = vec![(595.0, 842.0); paginas];
        Estado {
            nombre: "plano".into(),
            ajustes: lectura::Ajustes::default(),
            hojas: vista::Hojas::colocar(&medidas),
            medidas,
            pintadas: HashMap::new(),
            ultima_lista: Vec::new(),
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            zoom_cambiado: 0,
            marcas: Vec::new(),
            capas: HashMap::new(),
            donde: crate::lector_pdf_proyecto::DondeVa::solo_leer(Path::new("plano.pdf")),
            hechos: Vec::new(),
            deshechos: Vec::new(),
            anotando: false,
            tinta: Tinta::nueva(),
            cambio_en: None,
            ultimo_trazo: 0,
            arrastre: None,
            lateral: None,
            x_meta: None,
            pastilla_hasta: 0,
            panel: false,
            poniendo_marca: false,
            encima: DestinoRiel::Fuera,
            aviso: None,
            botones: Vec::new(),
            raton: (0.0, 0.0),
            error: None,
            exportando: false,
            hallar: HallarPdf::default(),
            renombre: Default::default(),
            voz: None,
            escuchar_al_llegar: false,
        }
    }

    fn pagina_con(texto: &str, v: f32) -> pixpin_pdf::texto::PaginaDeTexto {
        let n = texto.chars().count();
        pixpin_pdf::texto::PaginaDeTexto {
            texto: texto.into(),
            cajas: (0..n)
                .map(|i| Some([0.1 + i as f32 * 0.01, v, 0.11 + i as f32 * 0.01, v + 0.02]))
                .collect(),
        }
    }

    #[test]
    fn buscar_en_el_pdf_salta_a_la_hoja_y_al_renglon_de_la_palabra() {
        let mut e = estado(3);
        let m = marco();
        e.hallar.texto = Some(Ok(vec![
            pagina_con("portada", 0.1),
            pagina_con("nada", 0.1),
            pagina_con("aqui esta el Árbol", 0.5),
        ]));
        let ruta = std::path::Path::new("no-existe.pdf");
        e.hallar.caja.tecla(VK_F, false, true);
        for c in "arbol".chars() {
            assert!(caracter_de_las_cajas(&mut e, c, ruta));
        }
        assert_eq!(e.hallar.caja.busqueda.cuenta(), Some((1, 1)));
        assert!(recoger_texto(&mut e, m), "el salto pendiente pide repintar");
        // La palabra queda a la vista: por debajo de la caja de buscar y
        // dentro de la ventana.
        let y_palabra = e.hojas.arriba[2] + 0.5 * e.hojas.altos[2];
        let s = px(&e, m);
        assert!(
            y_palabra > e.y && y_palabra < e.y + m.alto / s,
            "{} {}",
            e.y,
            y_palabra
        );
        // Sin texto que buscar, la caja lo dice en vez de «sin resultados».
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(aviso_de_buscar(&e, &textos), None);
        e.hallar.texto = Some(Ok(vec![pagina_con("", 0.1)]));
        assert!(aviso_de_buscar(&e, &textos).is_some());
        e.hallar.texto = Some(Err(pixpin_pdf::texto::SinTexto::Cifrado));
        assert_eq!(
            aviso_de_buscar(&e, &textos),
            Some(textos.t("buscar-cifrado"))
        );
    }

    #[test]
    fn buscar_en_el_pdf_sin_cajas_al_menos_lleva_a_la_hoja() {
        let mut e = estado(3);
        let m = marco();
        e.hallar.texto = Some(Ok(vec![
            pagina_con("uno", 0.1),
            pagina_con("dos", 0.1),
            pixpin_pdf::texto::PaginaDeTexto {
                texto: "tres".into(),
                cajas: vec![None; 4],
            },
        ]));
        e.hallar.caja.tecla(VK_F, false, true);
        for c in "tres".chars() {
            caracter_de_las_cajas(&mut e, c, std::path::Path::new("x.pdf"));
        }
        recoger_texto(&mut e, m);
        assert_eq!(e.y, e.hojas.arriba[2]);
    }

    #[test]
    fn el_lector_de_pdf_solo_se_ofrece_para_pdf() {
        assert!(se_abre("plano.PDF"));
        assert!(se_abre("a.pdf"));
        assert!(!se_abre("a.docx"));
        assert!(!se_abre("pdf"));
    }

    #[test]
    fn al_proyecto_se_ofrece_solo_si_el_pdf_no_es_ya_de_uno() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let tiene = |e: &Estado| {
            filas_del_panel(e, &textos)
                .iter()
                .any(|(_, a)| matches!(a, Accion::AlProyecto))
        };
        let mut e = estado(3);
        assert!(tiene(&e), "un PDF suelto puede pasar a proyecto");
        // Caso negativo: el documento de un proyecto ya lo es.
        let raiz =
            std::env::temp_dir().join(format!("pixpin-lector-al-proyecto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let pdf = raiz.join("fuera/plano.pdf");
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        pixpin_proyecto::capas_del_pdf::al_proyecto(&raiz, &pdf, "plano", 3, 1, &|_| {
            raiz.join("no")
        })
        .unwrap();
        let doc = std::fs::read_dir(raiz.join("proyectos"))
            .unwrap()
            .flatten()
            .map(|d| d.path().join("archivos/doc-1.pdf"))
            .find(|p| p.is_file())
            .unwrap();
        e.donde = crate::lector_pdf_proyecto::DondeVa::de(&raiz, &doc, 2);
        assert!(!tiene(&e));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_hoja_se_centra_en_un_monitor_apaisado() {
        let mut e = estado(3);
        let m = marco();
        acotar(&mut e, m);
        let s = px(&e, m);
        let izquierda_en_pantalla = -e.x * s;
        let derecha_en_pantalla = (vista::ANCHO_HOJA - e.x) * s;
        assert!((izquierda_en_pantalla - (m.ancho - derecha_en_pantalla)).abs() < 1.0);
        assert!(
            (derecha_en_pantalla - izquierda_en_pantalla - 1000.0).abs() < 1.0,
            "a aumento 1 la hoja mide 1000 px"
        );
    }

    #[test]
    fn con_los_dos_espacios_se_aleja_hasta_ver_la_hoja_y_sus_margenes() {
        let mut e = estado(3);
        let m = Marco {
            ancho: 1280.0,
            ..marco()
        };
        e.ajustes.espacios = 3;
        e.zoom = 0.01;
        acotar(&mut e, m);
        let s = px(&e, m);
        let ancho_total = (vista::ANCHO_HOJA + 2.0 * 1050.0) * s;
        assert!(ancho_total <= m.ancho + 1.0, "cabe todo: {ancho_total}");
        assert!(e.zoom >= vista::ZOOM_MINIMO);
    }

    #[test]
    fn la_marca_se_pone_en_la_hoja_de_arriba_y_devuelve_alli() {
        let dir = std::env::temp_dir().join("pixpin-lector-pdf-marca");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("plano.pdf");
        let mut e = estado(5);
        e.y = e.hojas.arriba[2] + e.hojas.altos[2] * 0.25;
        poner_marca(&mut e, 0, &ruta);
        assert_eq!(e.marcas.len(), 1);
        assert_eq!(marcas::pagina_de(&e.marcas[0]), 2);
        assert!((marcas::alto_en_la_pagina(&e.marcas[0]) - 0.25).abs() < 0.01);
        let guardado = lectura::leer(&ruta);
        // Se guarda la linea del movil (centesimas), no el numero entero.
        assert_eq!(guardado.marcas, marcas::a_texto(&e.marcas));
        assert_eq!(
            marcas::pagina_de(&marcas::de_texto(&guardado.marcas)[0]),
            2,
            "y vuelve a la misma hoja"
        );
        e.y = 0.0;
        ir_a_la_marca(&mut e, 0);
        assert!((e.y - (e.hojas.arriba[2] + e.hojas.altos[2] * 0.25)).abs() < 2.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_trazo_cae_en_la_hoja_que_hay_bajo_el_raton_y_en_sus_unidades() {
        let mut e = estado(3);
        let m = marco();
        acotar(&mut e, m);
        e.y = e.hojas.arriba[1] - 10.0;
        let s = px(&e, m);
        // A la mitad de la hoja a lo ancho, 100 unidades por debajo de su
        // borde de arriba.
        e.raton = ((700.0 - e.x) * s, 110.0 * s);
        let (i, q) = en_la_hoja(&e, m).unwrap();
        assert_eq!(i, 1);
        assert!(
            (q.x - 700.0).abs() < 0.5 && (q.y - 100.0).abs() < 0.5,
            "{q:?}"
        );
    }

    #[test]
    fn deshacer_va_a_la_hoja_del_ultimo_trazo() {
        let mut e = estado(3);
        for i in [0usize, 2] {
            let c = e.capas.entry(i).or_default();
            e.tinta
                .trazar(c, &[Punto2::nuevo(10.0, 10.0), Punto2::nuevo(50.0, 50.0)]);
            e.hechos.push(i);
        }
        deshacer(&mut e);
        assert!(e.capas[&2].vacia(), "lo ultimo fue en la tercera hoja");
        assert!(!e.capas[&0].vacia());
        rehacer(&mut e);
        assert!(!e.capas[&2].vacia());
        let mut sin_nada = estado(1);
        deshacer(&mut sin_nada);
        rehacer(&mut sin_nada);
    }

    #[test]
    fn se_pide_la_hoja_a_la_resolucion_del_aumento_y_por_escalones() {
        let mut e = estado(2);
        let m = marco();
        let a1 = ancho_para(&e, 0, m);
        assert_eq!(a1 % ESCALON, 0);
        assert!(a1 >= 1000, "la hoja se ve a 1000 px: {a1}");
        e.zoom = 4.0;
        let a4 = ancho_para(&e, 0, m);
        assert!(a4 > a1);
        let alto = e.hojas.altos[0] / vista::ANCHO_HOJA * a4 as f32;
        assert!(
            a4 as f32 * alto <= PIXELES_POR_HOJA * 1.1,
            "con el tope de pixeles"
        );
    }

    #[test]
    fn mientras_cambia_el_aumento_no_se_pide_otra_vez_lo_que_ya_esta_pintado() {
        let (tx, rx) = mpsc::channel();
        let mut e = estado(3);
        let m = marco();
        acotar(&mut e, m);
        pedir_hojas(&mut e, &tx, m);
        let Pedido::Quiero(lista) = rx.try_recv().expect("pide las que se ven") else {
            panic!("se esperaba una lista");
        };
        assert!(!lista.is_empty());
        assert_eq!(lista[0].0, 0, "la que se ve primero");
        // La misma lista no se vuelve a mandar.
        pedir_hojas(&mut e, &tx, m);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn los_espacios_se_guardan_por_documento() {
        let dir = std::env::temp_dir().join("pixpin-lector-pdf-espacios");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("plano.pdf");
        let mut e = estado(1);
        alternar_espacio(&mut e, vista::ESPACIO_DERECHA, &ruta);
        assert_eq!(lectura::leer(&ruta).espacios, vista::ESPACIO_DERECHA);
        assert_eq!(limites(&e), (0.0, vista::ANCHO_HOJA + 1050.0));
        alternar_espacio(&mut e, vista::ESPACIO_DERECHA, &ruta);
        assert_eq!(lectura::leer(&ruta).espacios, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un PDF de verdad de dos hojas con dos marcadores en su indice, como
    /// lo exporta PixPin.
    fn pdf_con_indice() -> Vec<u8> {
        let img = pixpin_codec::ImagenRgba {
            ancho: 20,
            alto: 30,
            pixeles: [250, 250, 250, 255].repeat(600),
        };
        let base = pixpin_pdf::union::de_imagenes(&[img.clone(), img]).unwrap();
        let nada = |_: usize| None;
        let sin = |_: u64| None;
        let marcadores = [
            pixpin_pdf::con_anotaciones::Marcador {
                titulo: "⭐ Hoja 1".into(),
                pagina: 0,
                alto: 0.25,
            },
            pixpin_pdf::con_anotaciones::Marcador {
                titulo: "Capitulo 2".into(),
                pagina: 1,
                alto: 0.0,
            },
        ];
        let a = pixpin_pdf::con_anotaciones::Anotaciones {
            tinta: &nada,
            izquierda: 0.0,
            derecha: 0.0,
            marcadores: &marcadores,
            imagenes: &sin,
            letra: None,
        };
        pixpin_pdf::con_anotaciones::hacer(&base, &a).unwrap()
    }

    /// **Los marcadores vuelven** (Android v0.98.2): un PDF exportado desde
    /// PixPin trae los suyos en el indice y el lector los recoge la primera
    /// vez que lo abre, con su emoticono; los de otro programa, con 🔖.
    #[test]
    fn la_primera_vez_el_lector_recoge_los_marcadores_del_indice_y_no_mas() {
        let dir = std::env::temp_dir().join(format!("pixpin-lector-indice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("exportado.pdf");
        std::fs::write(&pdf, pdf_con_indice()).unwrap();
        let mut e = estado(2);
        assert!(recoger_el_indice(&mut e, &pdf));
        let resumen: Vec<(u32, &str)> = e
            .marcas
            .iter()
            .map(|m| (marcas::pagina_de(m), m.emoji.as_str()))
            .collect();
        assert_eq!(resumen, vec![(0, "⭐"), (1, "🔖")]);
        assert!((marcas::alto_en_la_pagina(&e.marcas[0]) - 0.25).abs() < 1e-3);
        assert!(e.ajustes.indice);
        // Quitadas a mano, no vuelven: ya se miro.
        e.marcas.clear();
        assert!(!recoger_el_indice(&mut e, &pdf));
        assert!(e.marcas.is_empty());
        // Caso negativo: con marcas propias no se mira el indice.
        let mut con = estado(2);
        con.marcas = marcas::con(&[], 0.5, 1.5, "❗", 1);
        assert!(recoger_el_indice(&mut con, &pdf));
        assert_eq!(con.marcas.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Poner o quitar un espacio ya no mueve la tinta** (Android v0.98.0):
    /// la de antes, sin marco, recibe el suyo con los espacios de antes de
    /// cambiarlos y se queda donde estaba; el fichero no se toca. En un PDF
    /// suelto (caso negativo) no se escribe nada.
    /// **Al abrir, la tinta sin marco recibe el suyo** (Android v0.98.5,
    /// `fijarLoViejo` en cuanto se saben las hojas): si luego llegan otros
    /// espacios (del movil, por la sincronizacion), ya no se corre. Sin
    /// fijarlo al abrir (caso negativo), los espacios nuevos la mueven.
    #[test]
    fn al_abrir_un_adjunto_su_tinta_sin_marco_recibe_el_suyo_y_no_se_corre() {
        let preparar = |etiqueta: &str| {
            let r = crate::anotado_del_adjunto::pruebas::raiz(etiqueta);
            let pdf =
                crate::anotado_del_adjunto::pruebas::con_adjunto(&r, "tesis.pdf", b"%PDF-1.4");
            let mut e = estado(1);
            e.donde = crate::lector_pdf_proyecto::DondeVa::de(&r, &pdf, 1);
            let h = e.donde.para_escribir(&pdf, 0);
            std::fs::create_dir_all(h.parent().unwrap()).unwrap();
            std::fs::write(&h, r#"{"elements":[{"id":"jDNyxAFkaxm2qZ-KsluKG","type":"freedraw","x":-800,"y":1000,"width":250,"height":0,"strokeWidth":1,"points":[{"x":0.0,"y":0.0},{"x":250.0,"y":0.0}]}]}"#).unwrap();
            (r, pdf, e)
        };
        let x = |e: &Estado, pdf: &Path| {
            e.donde
                .leer_capa(pdf, 0, e.ajustes.espacios, alto_de(&e.hojas, 0))
                .escena
                .caja()
                .unwrap()
                .0
        };
        let (r, pdf, mut e) = preparar("abrir-fija");
        al_saber_las_hojas(&e, &pdf);
        e.ajustes.espacios = 3;
        assert!(
            (x(&e, &pdf) - 99.2).abs() < 1.0,
            "fijada al abrir: {}",
            x(&e, &pdf)
        );
        let _ = std::fs::remove_dir_all(&r);
        // Caso negativo: sin fijarla al abrir, los espacios que llegan la mueven.
        let (r, pdf, mut e) = preparar("abrir-sin-fijar");
        e.ajustes.espacios = 3;
        assert!(
            (x(&e, &pdf) - 99.2).abs() > 1.0,
            "sin marco se corre: {}",
            x(&e, &pdf)
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_espacio_puesto_en_un_adjunto_deja_su_tinta_donde_estaba_como_el_movil() {
        let r = crate::anotado_del_adjunto::pruebas::raiz("espacio-adjunto");
        let pdf = crate::anotado_del_adjunto::pruebas::con_adjunto(&r, "tesis.pdf", b"%PDF-1.4");
        let mut e = estado(1);
        e.donde = crate::lector_pdf_proyecto::DondeVa::de(&r, &pdf, 1);
        let h = e.donde.para_escribir(&pdf, 0);
        std::fs::create_dir_all(h.parent().unwrap()).unwrap();
        std::fs::write(&h, r#"{"elements":[{"id":"jDNyxAFkaxm2qZ-KsluKG","type":"freedraw","x":-800,"y":1000,"width":250,"height":0,"strokeWidth":1,"points":[{"x":0.0,"y":0.0},{"x":250.0,"y":0.0}]}]}"#).unwrap();
        let x = |e: &mut Estado| {
            let (donde, espacios, alto) = (&e.donde, e.ajustes.espacios, alto_de(&e.hojas, 0));
            e.capas
                .entry(0)
                .or_insert_with(|| donde.leer_capa(&pdf, 0, espacios, alto))
                .escena
                .caja()
                .unwrap()
                .0
        };
        assert!(
            (x(&mut e) - 99.2).abs() < 1.0,
            "sin espacios: {}",
            x(&mut e)
        );
        alternar_espacio(&mut e, vista::ESPACIO_IZQUIERDA, &pdf);
        alternar_espacio(&mut e, vista::ESPACIO_DERECHA, &pdf);
        assert!(
            (x(&mut e) - 99.2).abs() < 1.0,
            "con los dos, donde estaba: {}",
            x(&mut e)
        );
        // Leida otra vez desde el disco, igual: manda su marco.
        e.capas.clear();
        assert!((x(&mut e) - 99.2).abs() < 1.0, "releida: {}", x(&mut e));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&h).unwrap()).unwrap();
        assert_eq!(
            v["elements"][0]["x"].as_f64(),
            Some(-800.0),
            "el fichero no cambia"
        );
        // Caso negativo: un PDF suelto no depende de los espacios.
        let mut suelto = estado(1);
        suelto.capas.insert(0, Capa::default());
        alternar_espacio(&mut suelto, vista::ESPACIO_DERECHA, &r.join("suelto.pdf"));
        assert!(suelto.capas.contains_key(&0));
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Un PDF de verdad para las muestras: el de un documento de texto, que
    /// `pixpin_docs::pdf` sabe escribir sin traer nada.
    fn pdf_de_muestra(ruta: &Path) {
        use pixpin_docs::documento::{Bloque, Clase, Documento, Trozo};
        let texto = "Memoria de calidades. La estructura será de hormigón armado con forjados \
                     unidireccionales; la cubierta, plana transitable con aislamiento térmico.";
        let mut bloques = vec![Bloque::nuevo(
            Clase::Titulo(1),
            vec![Trozo::llano("Proyecto de ejecución")],
        )];
        for _ in 0..40 {
            bloques.push(Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(texto)]));
        }
        let doc = Documento {
            titulo: "Proyecto".into(),
            bloques,
            ..Default::default()
        };
        std::fs::write(ruta, pixpin_docs::pdf::de_documento(&doc)).expect("pdf de muestra");
    }

    /// **El lector de PDF pintado de verdad**, y el PDF anotado que exporta,
    /// en PNG para mirarlos: dos hojas a la vista con los dos espacios para
    /// anotar, tinta en la hoja y en el margen, una marca y la barra de
    /// anotar. Necesita GPU: `cargo test -p pixpin --bin pixpinmax
    /// muestra_del_lector_de_pdf -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lector_de_pdf() {
        muestra_del_lector_de_pdf_inventado();
    }

    /// **La tinta del movil sobre un PDF del usuario, en el PC** (K16): una
    /// copia del PDF (`PIXPIN_PDF`) y la tinta de una hoja tal como la mando
    /// el movil (`PIXPIN_HOJA_TINTA`, `anot-<uid>-p<n>`, de la hoja
    /// `PIXPIN_HOJA`), la hoja entera con los dos espacios abiertos: leida
    /// tal cual (como la leia el PC hasta el 29-sep, dos veces y media mas
    /// grande sin espacios) y en las unidades de la capa del movil con sus
    /// espacios (`PIXPIN_ESPACIOS`, 0 si no hay `.espacios`). Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_tinta_del_movil_en_un_pdf -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y una copia de los datos del usuario"]
    fn muestra_de_la_tinta_del_movil_en_un_pdf() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let ruta = PathBuf::from(std::env::var("PIXPIN_PDF").expect("PIXPIN_PDF"));
        let tinta = PathBuf::from(std::env::var("PIXPIN_HOJA_TINTA").expect("PIXPIN_HOJA_TINTA"));
        let hoja: usize = std::env::var("PIXPIN_HOJA")
            .ok()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let documento = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
        let mut e = estado(0);
        e.medidas = documento.medidas();
        e.hojas = vista::Hojas::colocar(&e.medidas);
        let (w, h) = e.medidas[hoja];
        println!(
            "hoja {hoja}: {w}x{h} puntos -> {} de alto en unidades (1400 * {h}/{w} = {})",
            e.hojas.altos[hoja],
            1400.0 * h / w
        );
        // Los espacios del movil para este PDF (`anot-<uid>.espacios`, sin
        // fichero 0): con ellos se leen sus unidades (`capa_del_movil`).
        let espacios: u8 = std::env::var("PIXPIN_ESPACIOS")
            .ok()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        e.ajustes.espacios = 3;
        let u = lector_tinta::Unidades::de_la_capa_del_movil(espacios);
        // Y con lo dibujado antes en el PC pasado a esas unidades (una copia).
        let copia = carpeta.join(format!("k16-hoja{hoja}.excalidraw"));
        std::fs::copy(&tinta, &copia).expect("copia de la tinta");
        let _ = std::fs::remove_dir_all(lector_tinta::carpeta_de(&copia.with_extension("pdf")));
        crate::anotado_del_adjunto::lo_del_pc_a_la_capa_del_movil(
            &copia.with_extension("pdf"),
            hoja,
            &copia,
            u,
        );
        for (como, capa) in [
            ("tal-cual", Capa::leer(&tinta)),
            ("unidades-del-movil", Capa::leer_en(&tinta, u)),
            ("con-lo-del-pc-pasado", Capa::leer_en(&copia, u)),
        ] {
            e.capas.insert(hoja, capa);
            e.tinta.olvidar_lo_calculado();
            let caja = e.capas[&hoja].escena.caja().expect("hay tinta");
            println!("{como}: tinta (unidades de la hoja): {caja:?}");
            // La hoja entera en la ventana, con sus dos margenes.
            let (ancho, alto) = (1400u32, 1000u32);
            let m = Marco {
                ancho: ancho as f32,
                alto: alto as f32,
                e: 1.0,
                escala_por_cien: 100,
                area: pixpin_geom::Rect {
                    x: 0,
                    y: 0,
                    ancho,
                    alto,
                },
            };
            let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
            e.zoom = 1.0;
            let meta = m.alto / (e.hojas.altos[hoja] * 1.04);
            e.zoom *= meta / px(&e, m);
            let s = px(&e, m);
            e.x = vista::ANCHO_HOJA / 2.0 - m.ancho / s / 2.0;
            e.y = e.hojas.arriba[hoja] + e.hojas.altos[hoja] / 2.0 - m.alto / s / 2.0;
            e.pintadas.clear();
            for i in visibles(&e, m) {
                let img = documento
                    .renderizar(i as u32, ancho_para(&e, i, m))
                    .expect("hoja");
                let b = motor
                    .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
                    .expect("bitmap");
                e.pintadas.insert(i, (img.ancho, b));
            }
            motor
                .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                ancho: a,
                alto: b,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(format!("k16-hoja{hoja}-{como}.png"));
            std::fs::write(&salida, png).expect("guardar");
            println!("{como}: {}", salida.display());
        }
    }

    /// **El marco de la tinta sobre un PDF del usuario** (K21): la misma
    /// copia de datos que [`muestra_de_la_tinta_del_movil_en_un_pdf`]. Se
    /// pinta leida con la regla vieja, con el marco calculado para su escala
    /// (`anot-<uid>-p<n>.hoja`, que se escribe al lado para mirarlo), y
    /// reescrita en otra escala y otro origen con su marco: tiene que caer
    /// en el mismo sitio. Y, como caso negativo, esa ultima sin marco.
    /// Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_marco_de_la_tinta -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y una copia de los datos del usuario"]
    fn muestra_del_marco_de_la_tinta() {
        use crate::lector_pdf_proyecto::hoja_propia;
        use lector_tinta::Unidades;
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        use pixpin_sincro::anotado::MarcoDeLaHoja;

        let ruta = PathBuf::from(std::env::var("PIXPIN_PDF").expect("PIXPIN_PDF"));
        let tinta = PathBuf::from(std::env::var("PIXPIN_HOJA_TINTA").expect("PIXPIN_HOJA_TINTA"));
        let hoja: usize = std::env::var("PIXPIN_HOJA")
            .ok()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        let espacios: u8 = std::env::var("PIXPIN_ESPACIOS")
            .ok()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let documento = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
        let mut e = estado(0);
        e.medidas = documento.medidas();
        e.hojas = vista::Hojas::colocar(&e.medidas);
        let propia = hoja_propia(e.hojas.altos[hoja]);

        // El marco para la escala en que esta escrita (la de la capa del
        // movil con sus espacios), escrito al lado como lo escribiria el PC.
        let vieja = Unidades::de_la_capa_del_movil(espacios);
        let marco = vieja.marco_de(&propia);
        let fichero_marco = carpeta.join(format!("k21-hoja{hoja}.hoja"));
        std::fs::write(&fichero_marco, marco.a_texto()).expect("marco");
        println!(
            "marco ({}): {}",
            fichero_marco.display(),
            marco.a_texto().trim()
        );
        let leido = MarcoDeLaHoja::de_texto(&std::fs::read_to_string(&fichero_marco).unwrap())
            .expect("se entiende");
        let con_marco = Unidades::del_marco(&propia, &leido).expect("marco valido");

        // La misma tinta reescrita en otras unidades (0,6 a lo ancho y 0,7 a
        // lo alto, el cero en (200, -150)) con su marco.
        let otra = Unidades {
            ex: 0.6,
            ey: 0.7,
            dx: 200.0,
            dy: -150.0,
        };
        let copia = carpeta.join(format!("k21-hoja{hoja}-otra-escala.excalidraw"));
        let mut c = Capa::leer_en(&tinta, vieja);
        c.sucia = true;
        c.guardar_en(&copia, otra).expect("copia en otra escala");
        let marco_otra = otra.marco_de(&propia);
        println!("marco de la otra escala: {}", marco_otra.a_texto().trim());
        let con_marco_otra = Unidades::del_marco(
            &propia,
            &MarcoDeLaHoja::de_texto(&marco_otra.a_texto()).unwrap(),
        )
        .unwrap();

        let mut cajas = Vec::new();
        for (como, capa) in [
            ("regla-vieja", Capa::leer_en(&tinta, vieja)),
            ("con-su-marco", Capa::leer_en(&tinta, con_marco)),
            (
                "otra-escala-con-marco",
                Capa::leer_en(&copia, con_marco_otra),
            ),
            ("otra-escala-sin-marco", Capa::leer_en(&copia, vieja)),
        ] {
            e.capas.insert(hoja, capa);
            e.tinta.olvidar_lo_calculado();
            let caja = e.capas[&hoja].escena.caja().expect("hay tinta");
            println!("{como}: tinta (unidades de la hoja): {caja:?}");
            cajas.push(caja);
            let (ancho, alto) = (1400u32, 1000u32);
            let m = Marco {
                ancho: ancho as f32,
                alto: alto as f32,
                e: 1.0,
                escala_por_cien: 100,
                area: pixpin_geom::Rect {
                    x: 0,
                    y: 0,
                    ancho,
                    alto,
                },
            };
            let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
            e.ajustes.espacios = 3;
            e.zoom = 1.0;
            let meta = m.alto / (e.hojas.altos[hoja] * 1.04);
            e.zoom *= meta / px(&e, m);
            let s = px(&e, m);
            e.x = vista::ANCHO_HOJA / 2.0 - m.ancho / s / 2.0;
            e.y = e.hojas.arriba[hoja] + e.hojas.altos[hoja] / 2.0 - m.alto / s / 2.0;
            e.pintadas.clear();
            for i in visibles(&e, m) {
                let img = documento
                    .renderizar(i as u32, ancho_para(&e, i, m))
                    .expect("hoja");
                let b = motor
                    .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
                    .expect("bitmap");
                e.pintadas.insert(i, (img.ancho, b));
            }
            motor
                .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                ancho: a,
                alto: b,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(format!("k21-hoja{hoja}-{como}.png"));
            std::fs::write(&salida, png).expect("guardar");
            println!("{como}: {}", salida.display());
        }
        let igual = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| {
            [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)]
                .iter()
                .all(|(x, y)| (x - y).abs() < 0.5)
        };
        assert!(
            igual(cajas[0], cajas[1]),
            "con su marco, donde la regla vieja"
        );
        assert!(
            igual(cajas[1], cajas[2]),
            "en otra escala con su marco, en el mismo sitio"
        );
        assert!(!igual(cajas[1], cajas[3]), "sin marco, en otro sitio");
    }

    fn muestra_del_lector_de_pdf_inventado() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let dir = std::env::temp_dir().join("pixpin-muestra-lector-pdf");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("proyecto.pdf");
        pdf_de_muestra(&ruta);
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let m = Marco {
            ancho: 1600.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect {
                x: 0,
                y: 0,
                ancho: 1600,
                alto: 900,
            },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1600, 900).expect("superficie");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);

        let documento = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
        let mut e = estado(0);
        e.medidas = documento.medidas();
        e.hojas = vista::Hojas::colocar(&e.medidas);
        assert!(e.hojas.cuantas() >= 2, "la muestra tiene varias hojas");
        e.ajustes.espacios = 3;
        e.zoom = 0.45;
        acotar(&mut e, m);
        e.y = e.hojas.arriba[0] + e.hojas.altos[0] * 0.6;
        acotar(&mut e, m);
        for i in visibles(&e, m) {
            let img = documento
                .renderizar(i as u32, ancho_para(&e, i, m))
                .expect("hoja");
            let b = motor
                .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
                .expect("bitmap");
            e.pintadas.insert(i, (img.ancho, b));
        }
        // Tinta: un circulo sobre la hoja y una nota en el margen derecho,
        // con el lapiz del lienzo en rojo, y un rectangulo alrededor.
        use pixpin_motor2d::gesto::Herramienta;
        e.tinta.gesto.estilo.trazo = pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19);
        let capa = e.capas.entry(0).or_default();
        let circulo: Vec<Punto2> = (0..=40)
            .map(|k| {
                let a = k as f32 / 40.0 * std::f32::consts::TAU;
                Punto2::nuevo(700.0 + 260.0 * a.cos(), 1500.0 + 90.0 * a.sin())
            })
            .collect();
        e.tinta.trazar(capa, &circulo);
        let nota: Vec<Punto2> = (0..30)
            .map(|k| {
                let t = k as f32 * 20.0;
                Punto2::nuevo(1500.0 + t, 1450.0 + 40.0 * (t / 60.0).sin())
            })
            .collect();
        e.tinta.trazar(capa, &nota);
        crate::dibujo::teclas::elegir_herramienta(&mut e.tinta.gesto, Herramienta::Rectangulo);
        e.tinta.trazar(
            capa,
            &[Punto2::nuevo(380.0, 1360.0), Punto2::nuevo(1020.0, 1640.0)],
        );
        e.marcas = marcas::con(&[], 0.5, 0.7, "⭐", 1);
        e.anotando = true;
        motor
            .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png");
        let salida = carpeta.join("lector-pdf-anotando.png");
        std::fs::write(&salida, png).expect("guardar");
        println!("lector: {}", salida.display());

        // El PDF anotado: se escribe, se vuelve a abrir y su primera hoja
        // (con el margen y la tinta) sale a PNG.
        let mut tinta = HashMap::new();
        tinta.insert(0, pintado::ordenes_de_escena(&e.capas[&0].escena));
        let exportado = dir.join("anotado.pdf");
        exportar(
            &documento,
            &tinta,
            &e.hojas.altos,
            vista::espacios_del_pdf(3),
            &e.marcas,
            &exportado,
            &|_, _| {},
        )
        .expect("exporta");
        // Con la marca en el indice del PDF, si la hay.
        let crudo = std::fs::read(&exportado).expect("leido");
        assert_eq!(
            String::from_utf8_lossy(&crudo).contains("/Type /Outlines"),
            !e.marcas.is_empty(),
            "los marcadores viajan en el indice"
        );
        let otro = pixpin_pdf::Documento::abrir(&exportado).expect("el anotado abre");
        assert_eq!(
            otro.paginas() as usize,
            e.hojas.cuantas(),
            "todas las hojas"
        );
        let primera = otro.renderizar(0, 900).expect("hoja anotada");
        assert!(
            primera.ancho < primera.alto * 2,
            "con los dos margenes la hoja es mas ancha: {}x{}",
            primera.ancho,
            primera.alto
        );
        let png = pixpin_codec::imagen::codificar_png(&primera).expect("png");
        let salida = carpeta.join("lector-pdf-exportado.png");
        std::fs::write(&salida, png).expect("guardar");
        println!("exportado: {}", salida.display());
        drop(otro);
        drop(documento);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// La caja de lo que es claramente rojo en un RGBA: la tinta de las
    /// pruebas de grafito (el resto —la hoja, su numero, la otra tinta, que
    /// va en azul— no cuenta).
    /// Y cuanta tinta roja hay (la suma de lo que le falta de verde a cada
    /// pixel rojo): un trazo que se aclara o adelgaza la pierde aunque su
    /// caja siga igual.
    fn caja_de_lo_rojo(px: &[u8], ancho: u32) -> Option<((u32, u32, u32, u32), u64)> {
        let mut caja: Option<(u32, u32, u32, u32)> = None;
        let mut tinta = 0u64;
        for (i, c) in px.chunks_exact(4).enumerate() {
            let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
            let (x, y) = (i as u32 % ancho, i as u32 / ancho);
            // Fuera de la hoja y de la barra: el panel de la izquierda tiene
            // una muestra roja del color elegido.
            if r - g > 12 && r - b > 12 && x >= 400 && y >= 80 {
                tinta += (r - g) as u64;
                caja = Some(match caja {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
        caja.map(|c| (c, tinta))
    }

    /// Un garabato de `n` puntos en unidades de la hoja, en zigzag a lo
    /// ancho, que empieza en `(x, y)`.
    fn garabato(x: f32, y: f32, n: usize) -> Vec<Punto2> {
        (0..n)
            .map(|k| {
                let t = k as f32;
                Punto2::nuevo(x + (t * 7.0) % 500.0, y + 30.0 * (t / 9.0).sin() + t * 0.4)
            })
            .collect()
    }

    /// El lector con la tinta sin ajustes del usuario, dos hojas a la vista
    /// (el pie de la primera y la cabeza de la segunda), anotando con el
    /// grafito.
    fn lector_anotando(m: Marco) -> Estado {
        let mut e = estado(3);
        e.tinta = Tinta::sin_ajustes();
        e.zoom = 0.45;
        acotar(&mut e, m);
        e.y = e.hojas.arriba[0] + e.hojas.altos[0] * 0.6;
        acotar(&mut e, m);
        e.anotando = true;
        crate::dibujo::teclas::elegir_herramienta(
            &mut e.tinta.gesto,
            pixpin_motor2d::gesto::Herramienta::Grafito,
        );
        e
    }

    /// **El grafito del lector no cambia de tamano al soltar** (lo reporto
    /// el usuario: «dibujo algo y cuando suelto el dibujo se achica»). Se
    /// pinta fuera de pantalla el trazo a medias (largo, pintado mientras
    /// crece) y el mismo ya suelto, y la caja de su tinta y cuanta hay tienen
    /// que ser las mismas (antes, al soltar se recocia con otro paso y salia
    /// mas claro y fino: ver `grafito::paso_de`); sin nada mas anotado y con
    /// otras hojas anotadas con los mismos ids. Necesita GPU.
    #[test]
    #[ignore = "necesita GPU"]
    fn el_grafito_del_lector_no_cambia_de_tamano_al_soltar() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let m = Marco {
            ancho: 1600.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect {
                x: 0,
                y: 0,
                ancho: 1600,
                alto: 900,
            },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1600, 900).expect("superficie");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let rojo = pixpin_motor2d::ColorRgba::opaco(0.88, 0.10, 0.10);
        let azul = pixpin_motor2d::ColorRgba::opaco(0.10, 0.20, 0.90);
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(std::path::PathBuf::from);
        let pintar_y_medir = |e: &mut Estado, nombre: &str| {
            motor
                .dibujar(&fuera.destino, |p| pintar(e, p, m, &textos))
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            if nombre.is_empty() {
                return ((0, 0, 0, 0), 0);
            }
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            if let Some(c) = &carpeta {
                let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                    ancho,
                    alto,
                    pixeles: pixeles.clone(),
                })
                .expect("png");
                std::fs::write(c.join(format!("{nombre}.png")), png).expect("guardar");
            }
            caja_de_lo_rojo(&pixeles, ancho).expect("hay tinta roja a la vista")
        };
        for otras in [0usize, 12] {
            let mut e = lector_anotando(m);
            let y0 = e.y - e.hojas.arriba[0] + 300.0;
            // Lo anotado antes, en azul: en la otra hoja y en esta, con los
            // mismos ids (cada hoja numera los suyos desde 1).
            e.tinta.gesto.estilo.trazo = azul;
            for k in 0..otras {
                let c = e.capas.entry(1).or_default();
                e.tinta
                    .trazar(c, &garabato(200.0, 40.0 + k as f32 * 25.0, 80 + k * 10));
            }
            for k in 0..otras.saturating_sub(1) {
                let c = e.capas.entry(0).or_default();
                e.tinta.trazar(
                    c,
                    &garabato(200.0, y0 + 300.0 + k as f32 * 10.0, 80 + k * 10),
                );
            }
            e.tinta.hoja = Some(0);
            e.tinta.gesto.estilo.trazo = rojo;
            // Largo de verdad (unas 10.000 unidades: un garabato de media
            // hoja a este aumento), que es donde el paso crece con el largo,
            // y pintado mientras crece, de 25 en 25 puntos, como se dibuja.
            let puntos = garabato(300.0, y0, 1500);
            let c = e.capas.entry(0).or_default();
            e.tinta.empezar_trazo(c, &puntos[..2]);
            for trozo in puntos[2..].chunks(25) {
                let c = e.capas.get_mut(&0).unwrap();
                for q in trozo {
                    e.tinta.seguir_trazo(c, *q);
                }
                pintar_y_medir(&mut e, "");
            }
            let en_curso = pintar_y_medir(&mut e, &format!("lector-grafito-{otras}-en-curso"));
            let c = e.capas.get_mut(&0).unwrap();
            e.tinta.acabar_trazo(c, *puntos.last().unwrap());
            let suelto = pintar_y_medir(&mut e, &format!("lector-grafito-{otras}-suelto"));
            let otra_vez = pintar_y_medir(&mut e, &format!("lector-grafito-{otras}-otra-vez"));
            println!(
                "otras {otras}: en curso {en_curso:?}, suelto {suelto:?}, otra vez {otra_vez:?}"
            );
            let cerca = |(a, ta): ((u32, u32, u32, u32), u64),
                         (b, tb): ((u32, u32, u32, u32), u64)| {
                a.0.abs_diff(b.0) <= 3
                    && a.1.abs_diff(b.1) <= 3
                    && a.2.abs_diff(b.2) <= 3
                    && a.3.abs_diff(b.3) <= 3
                    && ta.abs_diff(tb) as f64 <= tb as f64 * 0.02
            };
            assert!(
                cerca(en_curso, suelto),
                "con {otras} trazos en otras hojas: {en_curso:?} -> {suelto:?}"
            );
            assert!(
                cerca(suelto, otra_vez),
                "con {otras}: {suelto:?} -> {otra_vez:?}"
            );
        }
    }
}
