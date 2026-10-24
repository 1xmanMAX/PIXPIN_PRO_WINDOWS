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
//! **La cola** (`cola`, 11-oct-2026): lo que se ve primero (la hoja mas
//! cercana al centro delante), luego las vecinas y al final el texto de la
//! hoja bajo el raton; cada lista nueva cancela lo que ya no se ve, lo
//! repetido se funde y lo ya entregado no se dibuja dos veces. Pasando hojas
//! deprisa no se pide nada (`MS_PASANDO`), y lo pintado se guarda dentro de
//! `PRESUPUESTO_MAPAS`, soltando lo mas lejano. **Muy de cerca** la hoja
//! entera se queda a `ANCHO_BASE` de fondo y lo que se ve llega nitido como
//! un trozo (`Documento::renderizar_trozo`); mientras, se ve lo de antes
//! estirado, sin huecos. Medido en `medir`.
//!
//! **Texto y enlaces** (`elegir`): leyendo, arrastrar sobre el texto lo
//! elige (doble clic la palabra, triple el renglon, Ctrl+C copia) y un
//! enlace del PDF lleva a su hoja o abre el navegador.
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
//!
//! # Word y PowerPoint
//!
//! Un `.docx` o un `.pptx` se abre **en este mismo lector**, con sus paginas
//! tal como las imprime Office: lo que cambia es de donde salen las hojas
//! (maquetadas y pintadas en vectores en este hilo, ver `office`), y todo lo
//! demas (el pin, el aumento, la tinta, buscar, escuchar, el negativo) es lo
//! de un PDF. Ademas: elegir texto con el raton y Ctrl+C, los
//! hipervinculos, imprimir (Ctrl+P) y, en un PowerPoint, pasar de una en una
//! y presentar (F5).

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

/// Lo que otro escribe en las capas con el lector abierto, a la vista.
mod al_dia;
mod cola;
mod elegir;
#[cfg(test)]
mod medir;
pub(crate) mod office;
mod voz;

pub(crate) use office::DeOffice;

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
/// **El presupuesto de los mapas de bits** de las hojas: lo que se ve entra
/// siempre; las vecinas, de la mas cercana a la mas lejana, mientras quepan.
/// Medido con la tesis de muestra (306 hojas): sin el, al acercar al maximo
/// se juntaban 110-160 MB de hojas a 16 megapixeles.
const PRESUPUESTO_MAPAS: usize = 96 << 20;
/// El ancho maximo de una hoja entera. Mas cerca, la hoja entera se queda a
/// este ancho (de fondo) y lo que se ve llega nitido como un trozo
/// (`cola::trozo_para`): una hoja entera a 7000 pixeles serian 200 MB y
/// varios segundos; el trozo de la pantalla, unos megas y lo de una hoja.
const ANCHO_BASE: u32 = 2048;
/// Pasar hojas deprisa (cambia la de arriba dos veces en menos de esto): no
/// se piden las que pasan, que llegarian cuando ya no se ven (medido: de
/// 4 a 12 hojas dibujadas en vano al pasar cien con la rueda).
const MS_PASANDO: u64 = 60;
/// El PDF se guarda en memoria para sacar el texto de una hoja solo si es
/// de este tamano o menos; mas grande, se lee cada vez.
const BYTES_PARA_GUARDAR: u64 = 48 << 20;
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
const VK_D: u32 = 0x44;
const VK_G: u32 = 0x47;
const VK_F: u32 = 0x46;
const VK_F2: u32 = 0x71;
const VK_F3: u32 = 0x72;
const VK_M: u32 = 0x4D;
const VK_N: u32 = 0x4E;
const VK_L: u32 = 0x4C;
const VK_S: u32 = 0x53;
const VK_Y: u32 = 0x59;
const VK_Z: u32 = 0x5A;
const VK_C: u32 = 0x43;
const VK_P: u32 = 0x50;
const VK_F5: u32 = 0x74;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;
/// `[` y `]` en un teclado espanol o ingles (`VK_OEM_4` y `VK_OEM_6`).
const VK_CORCHETE_ABRE: u32 = 0xDB;
const VK_CORCHETE_CIERRA: u32 = 0xDD;

/// Las paginas pedidas para la proxima vez que se abra cada fichero (la
/// cita del asistente: `abrir` con `pagina`). Se toman al abrir.
static PAGINA_AL_ABRIR: std::sync::Mutex<Vec<(std::path::PathBuf, u32)>> =
    std::sync::Mutex::new(Vec::new());

/// **Que la proxima vez que se abra `ruta` empiece en `pagina`** (desde 0),
/// en vez de donde se dejo.
pub fn abrir_en_la_pagina(ruta: &Path, pagina: u32) {
    let mut v = PAGINA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    v.retain(|(r, _)| r != ruta);
    v.push((ruta.to_path_buf(), pagina));
}

/// La pagina pedida para `ruta`, si la hay (y se olvida).
fn tomar_pagina_al_abrir(ruta: &Path) -> Option<u32> {
    let mut v = PAGINA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    let i = v.iter().position(|(r, _)| r == ruta)?;
    Some(v.remove(i).1)
}

/// Lo que se busca al abrir un fichero (el asistente: `abrir` con
/// `buscar`). Como [`PAGINA_AL_ABRIR`], se toma al abrir.
static BUSQUEDA_AL_ABRIR: std::sync::Mutex<Vec<(std::path::PathBuf, String)>> =
    std::sync::Mutex::new(Vec::new());

/// **Que la proxima vez que se abra `ruta` busque `texto`** y lo ensene.
pub fn abrir_buscando(ruta: &Path, texto: &str) {
    let mut v = BUSQUEDA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    v.retain(|(r, _)| r != ruta);
    v.push((ruta.to_path_buf(), texto.to_string()));
}

/// Los espacios para anotar que pidio el asistente (`ESPACIO_IZQUIERDA` o
/// `ESPACIO_DERECHA`), por fichero: el lector abierto los abre en su
/// siguiente vuelta; cerrado, ya estan en su `.pixpin-lectura`.
static ESPACIOS_PEDIDOS: std::sync::Mutex<Vec<(std::path::PathBuf, u8)>> =
    std::sync::Mutex::new(Vec::new());

/// **Que el lector de `ruta` tenga abierto el espacio `lado`** (no lo
/// cierra si ya lo estaba).
pub fn pedir_espacio(ruta: &Path, lado: u8) {
    let mut v = ESPACIOS_PEDIDOS.lock().unwrap_or_else(|e| e.into_inner());
    v.push((ruta.to_path_buf(), lado));
    if v.len() > 32 {
        v.remove(0);
    }
}

fn tomar_espacio_pedido(ruta: &Path) -> Option<u8> {
    let mut v = ESPACIOS_PEDIDOS.lock().unwrap_or_else(|e| e.into_inner());
    let i = v.iter().position(|(r, _)| r == ruta)?;
    Some(v.remove(i).1)
}

fn tomar_busqueda_al_abrir(ruta: &Path) -> Option<String> {
    let mut v = BUSQUEDA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    let i = v.iter().position(|(r, _)| r == ruta)?;
    Some(v.remove(i).1)
}

/// Los ficheros con un lector abierto ahora (15-oct, el asistente: «ve a la
/// pagina 12» con el PDF ya delante). Ese lector mira en cada vuelta si le
/// piden una pagina o una busqueda ([`abrir_en_la_pagina`],
/// [`abrir_buscando`]) y va ahi, sin abrir otra ventana.
static ABIERTOS: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Si hay un lector abierto con `ruta`.
pub fn esta_abierto(ruta: &Path) -> bool {
    ABIERTOS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|r| r == ruta)
}

/// Apunta el lector como abierto mientras viva.
struct Abierto(std::path::PathBuf);

impl Abierto {
    fn nuevo(ruta: &Path) -> Abierto {
        ABIERTOS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(ruta.to_path_buf());
        Abierto(ruta.to_path_buf())
    }
}

impl Drop for Abierto {
    fn drop(&mut self) {
        let mut v = ABIERTOS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = v.iter().position(|r| *r == self.0) {
            v.remove(i);
        }
    }
}

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

/// Si esto lo abre el lector con sus paginas de Office: los Word y los
/// PowerPoint de ahora (`.docx`, `.docm`, `.dotx`, `.dotm`, `.pptx`,
/// `.pptm`, `.ppsx`, `.ppsm`, `.potx`, `.potm`). Los de antes (`.doc`,
/// `.ppt`) no son un ZIP y van por otro camino.
pub fn se_abre_office(nombre: &str) -> bool {
    // Por la extension de verdad: un fichero llamado «docx» a secas no lo es.
    pixpin_office::tipo_de(&format!("x.{}", pixpin_docs::extension(nombre))).is_some()
}

/// **Abre un Word o un PowerPoint** en el lector, como un pin, en su propio
/// hilo. Leer el paquete son milisegundos; si no se deja (roto, cifrado,
/// otra cosa con esa extension), va a donde iba antes: el Word al lector de
/// documentos (`visor`) y el PowerPoint a presentarlo por PowerPoint.
pub fn lanzar_office(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let empezo = std::time::Instant::now();
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new().name("lector-office".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let documento = match pixpin_office::Documento::abrir(&ruta) {
            Ok(d) => d,
            Err(err) => {
                tracing::warn!(%err, ruta = %ruta.display(), "office: no se pudo leer; se abre como antes");
                como_antes(idioma, ubicacion, &ruta);
                return;
            }
        };
        tracing::info!(ms = empezo.elapsed().as_millis() as u64, tipo = ?documento.tipo(), "office: documento leido");
        let textos = Catalogo::nuevo(idioma);
        let mut documento = Some(documento);
        let hecho = crate::dispositivo_perdido::con_recursos("lector_office", |r| {
            // Si el dispositivo se pierde, la ventana se rehace y el
            // documento se vuelve a leer (el de antes se fue con ella).
            let documento = match documento.take() {
                Some(d) => d,
                None => pixpin_office::Documento::abrir(&ruta).map_err(|e| anyhow::anyhow!("{e}"))?,
            };
            abrir_con(r, &textos, &ubicacion, &ruta, Some(DeOffice { documento, idioma, empezo }))
        });
        if let Err(e) = hecho {
            tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el lector de Office");
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del lector de Office");
    }
}

/// A donde iba un Word o un PowerPoint antes de este lector.
fn como_antes(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let nombre = pixpin_docs::nombre(ruta);
    match pixpin_office::tipo_de(&nombre) {
        Some(pixpin_office::Tipo::PowerPoint) => {
            crate::diapositivas::lanzar_con_powerpoint(idioma, ubicacion, ruta)
        }
        _ if crate::visor::se_abre(&nombre) => crate::visor::lanzar(idioma, ubicacion, ruta),
        _ => {
            if let Err(e) = pixpin_shell::abrir::abrir(ruta) {
                tracing::warn!(?e, "office: tampoco se pudo abrir con su programa");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// El hilo que rasteriza

enum Pedido {
    /// Lo que hace falta, con su prioridad: sustituye a la lista anterior
    /// (lo que ya no se ve no se pinta; ver `cola`). `visto` es la ultima
    /// entrega que la ventana ya recogio.
    Quiero {
        lista: Vec<(cola::Prioridad, cola::Trabajo)>,
        visto: u64,
    },
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
    /// Una hoja entera o un trozo, con su numero de entrega; sin imagen si
    /// no se pudo dibujar (y no se vuelve a pedir).
    Hecho {
        serie: u64,
        trabajo: cola::Trabajo,
        img: Option<ImagenRgba>,
    },
    /// El texto y los enlaces de una hoja (o nada si no se pudo leer).
    Texto {
        serie: u64,
        hoja: usize,
        hoja_pdf: Option<elegir::HojaPdf>,
    },
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
    let medidas = documento.medidas();
    // La proporcion de cada hoja (alto entre ancho), para los trozos.
    let proporciones: Vec<f32> = medidas
        .iter()
        .map(|&(w, h)| if w > 0.0 && h > 0.0 { h / w } else { 1.414 })
        .collect();
    avisar(Llega::Medidas(medidas));
    let mut cola = cola::Cola::default();
    // El PDF en memoria para el texto de las hojas, leido la primera vez.
    let mut bytes: Option<Vec<u8>> = None;
    loop {
        // Sin trabajo se duerme hasta el siguiente pedido; con trabajo solo
        // mira si ha llegado una lista mas nueva.
        let primero = if cola.vacia() {
            match pedidos.recv() {
                Ok(p) => Some(p),
                Err(_) => {
                    tracing::debug!(
                        cancelados = cola.cancelados,
                        en_vano = cola.en_vano,
                        "lector-pdf: cola de hojas"
                    );
                    return;
                }
            }
        } else {
            None
        };
        for p in primero
            .into_iter()
            .chain(std::iter::from_fn(|| pedidos.try_recv().ok()))
        {
            match p {
                Pedido::Quiero { lista, visto } => cola.pedir(lista, visto),
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
        let Some(t) = cola.tomar() else { continue };
        match t {
            cola::Trabajo::Hoja { hoja, ancho } => {
                let img = documento
                    .renderizar(hoja as u32, ancho)
                    .map_err(|e| tracing::warn!(?e, hoja, "hoja del PDF que no se pudo dibujar"))
                    .ok();
                let (serie, _) = cola.entregar(t);
                avisar(Llega::Hecho {
                    serie,
                    trabajo: t,
                    img,
                });
            }
            cola::Trabajo::Trozo { hoja, .. } => {
                let proporcion = proporciones.get(hoja).copied().unwrap_or(1.414);
                let img = cola::trozo_en_fracciones(t, proporcion).and_then(|(f, w, h)| {
                    documento
                        .renderizar_trozo(hoja as u32, (f[0], f[1], f[2], f[3]), w, h)
                        .map_err(|e| {
                            tracing::warn!(?e, hoja, "trozo del PDF que no se pudo dibujar")
                        })
                        .ok()
                });
                let (serie, _) = cola.entregar(t);
                avisar(Llega::Hecho {
                    serie,
                    trabajo: t,
                    img,
                });
            }
            cola::Trabajo::Texto { hoja } => {
                let hoja_pdf = texto_de_la_hoja(documento.ruta(), &mut bytes, hoja);
                let (serie, _) = cola.entregar(t);
                avisar(Llega::Texto {
                    serie,
                    hoja,
                    hoja_pdf,
                });
            }
        }
    }
}

/// El texto y los enlaces de una hoja, del PDF guardado en memoria (se lee
/// la primera vez; si es muy grande, se lee cada vez y no se guarda).
fn texto_de_la_hoja(
    ruta: &Path,
    bytes: &mut Option<Vec<u8>>,
    hoja: usize,
) -> Option<elegir::HojaPdf> {
    let leer = |b: &[u8]| {
        let (texto, enlaces) = pixpin_pdf::texto::hoja_con_enlaces(b, hoja).ok()?;
        Some(elegir::HojaPdf { texto, enlaces })
    };
    if bytes.is_none() {
        let leido = std::fs::read(ruta).ok()?;
        if (leido.len() as u64) > BYTES_PARA_GUARDAR {
            return leer(&leido);
        }
        *bytes = Some(leido);
    }
    leer(bytes.as_deref()?)
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
    /// Lo de la voz en el engranaje (las voces de Microsoft).
    AjusteVoz(crate::leer_en_voz::AjusteVoz),
    /// Un boton de los mandos de los lados.
    Lado(lector::BotonLado),
    #[allow(dead_code)]
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
    /// Pantalla completa o de vuelta a pin (`lector_pin`).
    PantallaCompleta,
    /// Un boton de la caja de buscar (D9).
    Buscar(crate::buscador::Boton),
    /// Office: el dialogo de imprimir (Ctrl+P).
    Imprimir,
    /// PowerPoint: presentar desde la que se mira (F5).
    Presentar,
    /// Word: el documento reflujado en el lector de documentos.
    ModoLectura,
    /// Luna/sol en pantalla completa (en pin va en su barra).
    Oscuro,
    /// **Que suba sola** (el boton de las dos flechas, o D).
    Auto,
    /// Un boton de la barra de la pagina que sube sola.
    AutoBoton(lector::BotonAuto),
    /// Los marcos numerados: imprimirlos todos y quitarlos todos (el menu
    /// del plano).
    ImprimirMarcos,
    BorrarMarcos,
    /// PDF: «Ver como texto», en el lector de Word (`visor::como_texto`).
    ComoTexto,
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
    /// Office: eligiendo texto (lo lleva `office`).
    Elegir,
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
    ultima_lista: Vec<(cola::Prioridad, cola::Trabajo)>,
    /// **El trozo nitido** de cada hoja que se ve muy de cerca: su trabajo,
    /// su caja en unidades de la hoja y su bitmap. Se pinta encima de la hoja.
    trozos: HashMap<usize, (cola::Trabajo, RectF, ID2D1Bitmap1)>,
    /// La ultima entrega del hilo de las hojas que ya se recogio.
    visto: u64,
    /// Lo que el hilo no pudo dibujar: no se vuelve a pedir.
    fallidos: std::collections::HashSet<cola::Trabajo>,
    /// La hoja de arriba la ultima vez que cambio, y cuando: para saber si
    /// se estan pasando hojas deprisa (`MS_PASANDO`).
    paso: (usize, u64),
    pasando_hasta: u64,
    /// Elegir texto y seguir enlaces en un PDF (`elegir`).
    elegir: elegir::ElegirPdf,
    x: f32,
    y: f32,
    zoom: f32,
    zoom_cambiado: u64,
    marcas: Vec<Marca>,
    capas: HashMap<usize, Capa>,
    /// Como estaba en el disco cada capa leida: lo que escriba otro (el
    /// asistente, la sincronizacion) se ve sin reabrir (`al_dia`).
    al_dia: al_dia::AlDia,
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
    /// La ventana del lector: duena del menu «⋯».
    hwnd: isize,
    /// Los marcos numerados (`marcos_doc`), con la logica del plano.
    marcos: crate::marcos_doc::Marcos,
    /// Lo que guarda cada marcador ademas de su sitio: el zoom y el sitio
    /// exacto (`marcos_doc::Vistas`).
    vistas: crate::marcos_doc::Vistas,
    /// Fotografiando los marcos para imprimirlos en grupo.
    fotos: Option<FotosDeMarcos>,
    /// Pintar solo las hojas, sin interfaz (para esas fotos).
    fotografiando: bool,
    poniendo_marca: bool,
    /// El contenido en negativo (N, 10-oct-2026): solo al pintar, sin
    /// tocar el documento; la interfaz y la tinta quedan con sus colores.
    en_negativo: bool,
    /// **Modo oscuro** (luna/sol de la barra, `crate::oscuro`): la hoja de
    /// un PDF con la luz invertida y el tono conservado al pintarla (sin
    /// tocar la cache de hojas); la de un Word o un PowerPoint, con el papel
    /// oscuro y su texto claro (`PintorOffice::poner_oscuro`).
    oscuro: bool,
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
    /// Se pidio el menu de las voces de Microsoft (lo saca el bucle).
    elegir_voz: bool,
    /// Engancharse a la voz de la app en cuanto llegue el texto: el PDF se
    /// estaba escuchando con el lector cerrado.
    engancharse: bool,
    /// **Como un pin** (`lector_pin`): fuera de pantalla completa la pastilla
    /// de arriba no se pinta (lo suyo va en la barra de fuera).
    en_pin: bool,
    /// Pantalla completa (o volver a pin) pedida desde dentro: la pastilla o Esc.
    pedir_pantalla_completa: bool,
    /// **Un Word o un PowerPoint** (`office`): sus hojas maquetadas en vez
    /// de las del hilo del PDF.
    office: Option<office::HojasOffice>,
    /// **La pagina que sube sola** (la barra de abajo), si esta puesta.
    auto: Option<pixpin_docs::auto_desplazar::Marcha>,
    /// Las palabras del documento, en cuanto llega su texto: hasta entonces
    /// la barra dice «Contando las palabras…» y no corre.
    palabras: Option<usize>,
    /// Donde se recuerda la velocidad (`lector::ppm_guardadas`).
    ubicacion: Ubicacion,
    /// «Ver como texto» en marcha: lo que dira su hilo.
    como_texto: Option<mpsc::Receiver<Result<PathBuf, crate::visor::como_texto::NoSale>>>,
}

pub fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    ruta: &Path,
) -> Result<()> {
    abrir_con(recursos, textos, ubicacion, ruta, None)
}

/// El lector, con las hojas de un PDF o (con `de_office`) las de un Word o
/// un PowerPoint.
fn abrir_con(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    ruta: &Path,
    de_office: Option<DeOffice>,
) -> Result<()> {
    // Las hojas de Office se pintan con el motor de esta ventana: el pintor,
    // antes que la ventana, y si no se puede (sin letras del sistema), como
    // antes.
    let office = match de_office {
        Some(d) => {
            let idioma = d.idioma;
            match office::HojasOffice::nuevo(d, &recursos.motor()) {
                Some(o) => Some(o),
                None => {
                    tracing::warn!("office: sin pintor (letras del sistema); se abre como antes");
                    como_antes(idioma, ubicacion.clone(), ruta);
                    return Ok(());
                }
            }
        }
        None => None,
    };
    // Cuenta como abierto para los grupos de ventanas (H9) mientras viva.
    let _grupo = crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lector {
        ruta: ruta.to_path_buf(),
    });
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let (area_pin, _) = crate::lector_pin::area_inicial();
    // Un Word o un PowerPoint de antes convertido lleva la etiqueta del
    // suyo (`.doc`), no la de la copia (`office_convertido`).
    let mut pin =
        crate::lector_pin::PinLector::nuevo(area_pin, monitor.escala_por_cien as f32 / 100.0)
            .con_extension(&pixpin_docs::extension(&pixpin_docs::nombre(
                &pixpin_proyecto::anotado::original_de(ruta).unwrap_or_else(|| ruta.to_path_buf()),
            )));
    let mut area = pin.area;
    let mut marco = Marco {
        ancho: area.ancho as f32,
        alto: area.alto as f32,
        e: monitor.escala_por_cien as f32 / 100.0,
        escala_por_cien: monitor.escala_por_cien,
        area,
    };
    let titulo = if office.is_some() {
        "lector-office-titulo"
    } else {
        "lector-pdf-titulo"
    };
    let mut ventana = VentanaOverlay::nueva_normal(area, &textos.t(titulo))
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
    // Como un pin pero sin fijar (lo fija su pin): al frente, una ventana
    // mas entre las demas, y su barra fuera.
    pin.al_nacer(&ventana);
    pin.crear_barra(&motor, &recursos.d3d(), &ventana);
    // El nombre y el atajo de sus botones, para el globo de la barra.
    pin.poner_pistas(vec![
        (1, textos.t("lector-escuchar")),
        (4, textos.t("lector-auto")),
        (2, textos.t("lector-mas")),
        (3, textos.t("lector-presentar")),
    ]);

    let (tx_pedidos, rx_pedidos) = mpsc::channel::<Pedido>();
    let (tx_llega, rx_llega) = mpsc::channel::<Llega>();
    let hwnd = ventana.handle().0 as isize;
    // Antes que nada: de que proyecto es, y lo anotado antes junto al PDF
    // adoptado en sus hojas. Se pinta su copia limpia si la tiene.
    let donde = crate::lector_pdf_proyecto::DondeVa::de(ubicacion.raiz(), ruta, ahora_ms() as i64);
    let ruta_hilo = donde.documento(ruta);
    // Un Office no tiene hilo de hojas: se maquetan y pintan aqui.
    let hilo = if office.is_none() {
        Some(
            std::thread::Builder::new()
                .name("lector-pdf-hojas".into())
                .spawn(move || hilo_de_hojas(ruta_hilo, rx_pedidos, tx_llega, hwnd))
                .context("sin hilo para dibujar las hojas")?,
        )
    } else {
        drop((rx_pedidos, tx_llega));
        None
    };

    // Con lo que viaja del chat encima, si es un adjunto (v0.96).
    let mut ajustes = crate::anotado_del_adjunto::leer(ruta);
    // Abierto desde una cita del asistente: en esa pagina, arriba.
    if let Some(p) = tomar_pagina_al_abrir(ruta) {
        ajustes.pagina = f64::from(p);
    }
    let mut e = Estado {
        nombre: pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta)),
        marcas: marcas::de_texto(&ajustes.marcas),
        zoom: ajustes.zoom,
        ajustes,
        medidas: Vec::new(),
        hojas: vista::Hojas::default(),
        pintadas: HashMap::new(),
        ultima_lista: Vec::new(),
        trozos: HashMap::new(),
        visto: 0,
        fallidos: Default::default(),
        paso: (0, 0),
        pasando_hasta: 0,
        elegir: Default::default(),
        x: 0.0,
        y: 0.0,
        zoom_cambiado: 0,
        capas: HashMap::new(),
        al_dia: Default::default(),
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
        hwnd,
        marcos: crate::marcos_doc::Marcos::de(Some(ruta)),
        vistas: crate::marcos_doc::Vistas::de_documento(Some(ruta)),
        fotos: None,
        fotografiando: false,
        panel: false,
        poniendo_marca: false,
        en_negativo: false,
        oscuro: false,
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
        elegir_voz: false,
        // Si se esta escuchando este PDF con su lector cerrado: se engancha
        // al llegar el texto (se pide abajo).
        engancharse: crate::leer_en_voz::suena(ruta),
        en_pin: true,
        pedir_pantalla_completa: false,
        office,
        auto: None,
        palabras: None,
        ubicacion: ubicacion.clone(),
        como_texto: None,
    };

    // El nombre de la pastilla: el del mensaje del chat si es de uno.
    e.nombre = pixpin_docs::sin_extension(&e.renombre.visto);
    // El indice de marcadores es cosa de un PDF.
    if e.office.is_none() && recoger_el_indice(&mut e, ruta) {
        guardar_ajustes(&mut e, ruta);
    }
    // Se estaba escuchando con el lector cerrado: hace falta su texto para
    // engancharse (`voz::vuelta`).
    if e.engancharse {
        voz::pedir_el_texto(&mut e, ruta);
    }
    // Abierto por el asistente para ensenar un texto: la caja de buscar ya
    // escrita; al llegar el texto (`recoger_texto`) salta a la coincidencia
    // desde la pagina pedida y la marca, como con Ctrl+F.
    // Pruebas: imprimir los marcos solo, a los 2 s de abrir (con
    // `PIXPIN_IMPRIMIR_A_PDF`, a un fichero), sin Ctrl+P de verdad.
    let mut imprimir_al_abrir =
        std::env::var_os("PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR").map(|_| ahora_ms() + 2000);
    if let Some(q) = tomar_busqueda_al_abrir(ruta) {
        e.hallar.caja.buscar_ya(&q);
        voz::pedir_el_texto(&mut e, ruta);
    }
    let mut hay_que_pintar = true;
    let mut vivo = true;
    // La ruta cambia si se renombra un PDF suelto (D5).
    let mut ruta_viva = ruta.to_path_buf();
    let mut ruta_nueva: Option<PathBuf> = None;
    // Abierto: el asistente lo lleva a una pagina o un texto sin otra ventana.
    let _abierto = Abierto::nuevo(ruta);
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
        // Lo que pide el asistente con el lector ya abierto (15-oct): ir a
        // una pagina o buscar un texto, como al abrirlo.
        if let Some(p) = tomar_pagina_al_abrir(ruta) {
            office::ir_a(&mut e, p as usize, marco);
            hay_que_pintar = true;
        }
        if let Some(q) = tomar_busqueda_al_abrir(ruta) {
            e.hallar.caja.buscar_ya(&q);
            voz::pedir_el_texto(&mut e, ruta);
            // Con el texto ya leido, busca ahora; si no, al llegar.
            rebuscar(&mut e);
            hay_que_pintar = true;
        }
        // La pagina que sube sola y el PDF pasado a texto.
        if subir_sola(&mut e, marco) {
            hay_que_pintar = true;
        }
        if recoger_como_texto(&mut e, textos, ubicacion) {
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
        // Un Office: unas cuantas hojas mas maquetadas en cada vuelta.
        if office::vuelta(&mut e, ruta, marco) {
            hay_que_pintar = true;
        }
        for (h, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if pin.es_de_la_barra(h) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(crate::lector_pin::AccionPin::Propio(id)) => {
                        if let Some(a) = accion_del_pin(id) {
                            hacer(&mut e, a, textos, ruta, ubicacion, &tx_pedidos, marco);
                        }
                    }
                    Some(crate::lector_pin::AccionPin::Nombre) => hacer(
                        &mut e,
                        Accion::Renombrar,
                        textos,
                        ruta,
                        ubicacion,
                        &tx_pedidos,
                        marco,
                    ),
                    Some(crate::lector_pin::AccionPin::PantallaCompleta) => {
                        pin.alternar_pantalla_completa(&mut ventana)
                    }
                    Some(crate::lector_pin::AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(crate::lector_pin::AccionPin::Cerrar) => vivo = false,
                    None => {}
                }
                hay_que_pintar = true;
                continue;
            }
            if h != ventana.handle() {
                continue;
            }
            // Estirar por los bordes es del pin.
            if pin.evento_ventana(&evento, &mut ventana).consumido {
                hay_que_pintar = true;
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
                // Los marcos numerados (`marcos_doc`), como en el plano: con el
                // modo puesto, arrastrar en el documento pone uno; los botones y
                // el riel siguen siendo suyos.
                EventoOverlay::BotonPulsado(p)
                    if e.marcos.activo && !sobre_la_interfaz(&e, marco, local(p)) =>
                {
                    e.raton = local(p);
                    // Una ficha de abajo: a ese marco; si no, uno nuevo.
                    if let Some(i) = e.marcos.ficha_en(e.raton, marco.alto, marco.e) {
                        ir_al_marco(&mut e, marco, i);
                    } else {
                        e.marcos.pulsar(doc_del_raton(&e, marco), 0);
                        ventana.capturar_raton();
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::RatonMovido(p) if e.marcos.arrastrando() => {
                    e.raton = local(p);
                    e.marcos.mover(doc_del_raton(&e, marco));
                    hay_que_pintar = true;
                }
                EventoOverlay::BotonSoltado(p) if e.marcos.arrastrando() => {
                    e.raton = local(p);
                    e.marcos
                        .soltar(doc_del_raton(&e, marco), px(&e, marco) as f64);
                    ventana.soltar_raton();
                    hay_que_pintar = true;
                }
                // I: el modo; Supr: quitar el ultimo; Ctrl+P con marcos: todos.
                EventoOverlay::Tecla {
                    vk,
                    ctrl,
                    shift: false,
                    ..
                } if !escribiendo(&e)
                    && ((vk == crate::marcos_doc::VK_I && !ctrl && !e.anotando)
                        || (vk == crate::marcos_doc::VK_SUPR && !ctrl && e.marcos.activo)
                        || (vk == VK_P && ctrl && !e.marcos.lista.is_empty())) =>
                {
                    match e.marcos.tecla(vk, ctrl) {
                        Some(crate::marcos_doc::Pedido::Imprimir) => {
                            imprimir_marcos(&mut e, ruta);
                        }
                        Some(crate::marcos_doc::Pedido::Alternado) if e.marcos.activo => {
                            e.poniendo_marca = false;
                        }
                        _ => {}
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::RatonMovido(p) => {
                    e.raton = local(p);
                    mover_raton(&mut e, marco);
                    // Office: la mano en un hipervinculo, la barra en el texto.
                    if let Some(forma) = office::cursor(&mut e, marco) {
                        ventana.poner_cursor(forma);
                    }
                    // PDF: la mano en un enlace, la barra en el texto.
                    if let Some(forma) = elegir::cursor(&mut e, marco) {
                        ventana.poner_cursor(forma);
                    }
                }
                // Office: imprimir y presentar, que necesitan la ventana o
                // donde viven los datos.
                EventoOverlay::Tecla {
                    vk: VK_P,
                    ctrl: true,
                    shift: false,
                    ..
                } if e.office.is_some() => {
                    office::imprimir(&mut e, ventana.handle().0 as isize, ruta, textos)
                }
                EventoOverlay::Tecla { vk: VK_F5, .. }
                    if e.office.is_some() && !escribiendo(&e) =>
                {
                    hacer(
                        &mut e,
                        Accion::Presentar,
                        textos,
                        ruta,
                        ubicacion,
                        &tx_pedidos,
                        marco,
                    )
                }
                EventoOverlay::Tecla {
                    vk: VK_C,
                    ctrl: true,
                    ..
                } if e.office.is_some() && !escribiendo(&e) => {
                    office::copiar(&mut e, Some(textos));
                }
                EventoOverlay::Tecla {
                    vk: VK_C,
                    ctrl: true,
                    ..
                } if e.office.is_none() && !escribiendo(&e) && e.elegir.seleccion.is_some() => {
                    elegir::copiar(&mut e, textos);
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
                } if !e.anotando && !escribiendo(&e) => {
                    // Escuchar cierra la pagina que sube sola.
                    e.auto = None;
                    voz::escuchar(&mut e, textos, ruta, marco)
                }
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
                    // G: el menu «⋯» (la tecla solo lo pide; se abre aqui,
                    // donde estan los textos).
                    if std::mem::take(&mut e.panel) {
                        hacer(
                            &mut e,
                            Accion::Engranaje,
                            textos,
                            ruta,
                            ubicacion,
                            &tx_pedidos,
                            marco,
                        );
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

        // Pantalla completa pedida desde dentro (la pastilla o Esc).
        if e.pedir_pantalla_completa {
            e.pedir_pantalla_completa = false;
            pin.alternar_pantalla_completa(&mut ventana);
        }
        // «Voz de Microsoft» en el engranaje: el menu de las voces.
        if std::mem::take(&mut e.elegir_voz) {
            if let Some(aviso) =
                crate::leer_en_voz::elegir_voz(ventana.handle(), textos, e.ubicacion.raiz())
            {
                e.aviso = Some((aviso, ahora_ms() + 4000));
            }
            hay_que_pintar = true;
        }
        // Imprimir pedido desde el panel (necesita la ventana).
        if e.office
            .as_mut()
            .is_some_and(|o| std::mem::take(&mut o.pedir_imprimir))
        {
            office::imprimir(&mut e, ventana.handle().0 as isize, ruta, textos);
            hay_que_pintar = true;
        }
        // Movida o estirada desde fuera (AltSnap, Win+flecha): el pin la sigue.
        pin.seguir_ventana(&mut ventana);
        // **La ventana cambio** (estirada, llevada, a pantalla completa): la
        // superficie y el marco, a su medida nueva; las hojas se repintan al
        // ancho nuevo.
        if pin.area != area {
            if (pin.area.ancho, pin.area.alto) != (area.ancho, area.alto) {
                let _ = superficie.redimensionar(pin.area.ancho, pin.area.alto);
            }
            area = pin.area;
            marco = Marco {
                ancho: area.ancho as f32,
                alto: area.alto as f32,
                area,
                ..marco
            };
            hay_que_pintar = true;
        }
        e.en_pin = !pin.pantalla_completa;
        // Luna/sol (de este pin o de otro): a repintar con el modo nuevo.
        if pin.actualizar(
            &motor,
            &crate::lector_pin::nombre_limpio(&nombre_del_pin(&e)),
            &botones_del_pin(&e),
        ) {
            hay_que_pintar = true;
        }
        e.oscuro = pin.oscuro();

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
        // Se dejo de pasar hojas deprisa: toca pedir las que se ven.
        if e.pasando_hasta > 0 && ahora >= e.pasando_hasta {
            e.pasando_hasta = 0;
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
        // Lo que el asistente o la sincronizacion escribio en una capa
        // abierta, a la vista (sin pisar lo que se este anotando).
        if al_dia::mirar(&mut e, ruta) {
            hay_que_pintar = true;
        }
        // El asistente puso sus notas fuera de la hoja: el espacio para
        // anotar de ese lado, abierto.
        if let Some(lado) = tomar_espacio_pedido(ruta)
            && e.ajustes.espacios & lado == 0
        {
            alternar_espacio(&mut e, lado, ruta);
            hay_que_pintar = true;
        }
        let sucias = e.capas.values().any(|c| c.sucia) && !e.tinta.trazando();
        if sucias && ahora > e.ultimo_trazo + MS_PARA_GUARDAR {
            guardar_capas(&mut e, ruta);
        }

        if imprimir_al_abrir.is_some_and(|t| ahora_ms() > t) {
            imprimir_al_abrir = None;
            imprimir_marcos(&mut e, ruta);
        }
        if paso_de_las_fotos(&mut e, marco) {
            hay_que_pintar = true;
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            acotar(&mut e, marco);
            if e.office.is_none() {
                pedir_hojas(&mut e, &tx_pedidos, marco);
            }
            cargar_capas_visibles(&mut e, ruta, marco);
            al_dia::apuntar(&mut e, ruta);
            al_dia::apuntar_turnos(&mut e);
            if let Ok(destino) = superficie.empezar(&motor) {
                let t = std::time::Instant::now();
                let dibujado =
                    motor.dibujar(&destino, |p: &Pintor| pintar(&mut e, p, marco, textos));
                let _ = superficie.presentar();
                // Office se pinta en vectores en cada fotograma: si uno
                // tarda o falla, que quede escrito.
                if e.office.is_some() {
                    if let Err(err) = &dibujado {
                        tracing::warn!(
                            ?err,
                            zoom = e.zoom,
                            "office: el fotograma no se pudo pintar"
                        );
                    } else if t.elapsed().as_millis() > 50 {
                        tracing::info!(
                            ms = t.elapsed().as_millis() as u64,
                            zoom = e.zoom,
                            "office: fotograma lento"
                        );
                    }
                }
            }
        }
        let tope = if e.x_meta.is_some() || subiendo(&e) {
            16
        } else {
            50
        };
        // Con hojas de Office por maquetar, la siguiente vuelta enseguida.
        let tope = if e.office.as_ref().is_some_and(|o| o.maquetando()) {
            1
        } else {
            tope
        };
        let tope = e.tinta.mano.tope_forma_ms().map_or(tope, |t| t.min(tope));
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }

    let ruta: &Path = &ruta_viva;
    guardar_capas(&mut e, ruta);
    if !e.hojas.arriba.is_empty() {
        e.ajustes.pagina = e.hojas.sitio(e.y);
    }
    e.ajustes.zoom = e.zoom;
    guardar_ajustes(&mut e, ruta);
    // La voz sigue con el lector cerrado (es de la app): solo se suelta, ya
    // guardado lo de aqui, y desde ahora el verde lo apunta ella.
    e.voz = None;
    // Soltar el canal despierta al hilo, que ve que no hay nadie y acaba.
    drop(tx_pedidos);
    if e.exportando
        && let Some(hilo) = hilo
    {
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
        Llega::Hecho {
            serie,
            trabajo,
            img,
        } => {
            e.visto = e.visto.max(serie);
            let Some(img) = img else {
                e.fallidos.insert(trabajo);
                return;
            };
            let i = trabajo.hoja();
            if let cola::Trabajo::Trozo { .. } = trabajo {
                // El trozo va encima de su hoja, en su caja de unidades.
                let proporcion = e
                    .hojas
                    .altos
                    .get(i)
                    .map_or(1.414, |a| a / vista::ANCHO_HOJA);
                let Some((f, _, _)) = cola::trozo_en_fracciones(trabajo, proporcion) else {
                    return;
                };
                let alto = alto_de(&e.hojas, i);
                let caja = RectF {
                    x: f[0] * vista::ANCHO_HOJA,
                    y: f[1] * alto,
                    ancho: f[2] * vista::ANCHO_HOJA,
                    alto: f[3] * alto,
                };
                match motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles) {
                    Ok(b) => {
                        e.trozos.insert(i, (trabajo, caja, b));
                    }
                    Err(err) => tracing::warn!(?err, hoja = i, "no se pudo subir el trozo"),
                }
                return;
            }
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
        Llega::Texto {
            serie,
            hoja,
            hoja_pdf,
        } => {
            e.visto = e.visto.max(serie);
            // Sin texto (cifrado, roto) queda vacia: tampoco se vuelve a pedir.
            e.elegir
                .hojas
                .insert(hoja, std::sync::Arc::new(hoja_pdf.unwrap_or_default()));
            if e.elegir.falta == Some(hoja) {
                e.elegir.falta = None;
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
                    // A la Salida y no a la carpeta: desde ahi se arrastra,
                    // se manda o se abre la carpeta si se quiere.
                    let texto = textos.t_args("visor-guardado", &args);
                    crate::salida::mostrar(vec![destino], textos.t("salida-titulo-exportado"));
                    texto
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

/// El ancho que pide el aumento de ahora para una hoja, por escalones y sin
/// mas tope que el de `Windows.Data.Pdf`: el de los trozos nitidos.
fn ancho_deseado(e: &Estado, m: Marco) -> u32 {
    let deseado = (vista::ANCHO_HOJA * px(e, m)).ceil().max(1.0) as u32;
    (deseado.div_ceil(ESCALON) * ESCALON).clamp(ESCALON, pixpin_pdf::ANCHO_MAXIMO)
}

/// Alto entre ancho de la hoja `i`.
fn proporcion_de(e: &Estado, i: usize) -> f32 {
    e.hojas
        .altos
        .get(i)
        .map_or(1.414, |a| a / vista::ANCHO_HOJA)
        .max(0.05)
}

/// El ancho al que se rasteriza **la hoja entera** `i`: el que pide el
/// aumento hasta `ANCHO_BASE` y el tope de pixeles (una hoja muy larga).
/// Mas cerca, lo que se ve llega como un trozo (ver `lista_de_trabajos`).
fn ancho_para(e: &Estado, i: usize, m: Marco) -> u32 {
    let cabe = (PIXELES_POR_HOJA / proporcion_de(e, i)).sqrt() as u32;
    ancho_deseado(e, m).min(ANCHO_BASE).min(cabe).max(ESCALON)
}

/// Los bytes de una hoja rasterizada a `ancho`.
fn bytes_de(ancho: u32, proporcion: f32) -> usize {
    ancho as usize * cola::alto_en_pixeles(ancho, proporcion) as usize * 4
}

/// Lo que se ve de la hoja `i`, en fracciones de ella (x0, y0, x1, y1).
fn vista_de_la_hoja(e: &Estado, i: usize, m: Marco) -> [f32; 4] {
    let s = px(e, m);
    let arriba = e.hojas.arriba.get(i).copied().unwrap_or(0.0);
    let alto = alto_de(&e.hojas, i).max(1.0);
    [
        e.x / vista::ANCHO_HOJA,
        (e.y - arriba) / alto,
        (e.x + m.ancho / s) / vista::ANCHO_HOJA,
        (e.y + m.alto / s - arriba) / alto,
    ]
}

/// Las hojas que se ven.
fn visibles(e: &Estado, m: Marco) -> std::ops::Range<usize> {
    let s = px(e, m);
    e.hojas.visibles(e.y, e.y + m.alto / s)
}

/// A cuantas hojas de lo que se ve esta la `i` (0: se ve).
fn distancia(vis: &std::ops::Range<usize>, i: usize) -> usize {
    if i < vis.start {
        vis.start - i
    } else if i >= vis.end {
        i + 1 - vis.end
    } else {
        0
    }
}

/// **Lo que hace falta pedir al hilo**, con su prioridad (`cola`):
///
/// - lo que se ve, la hoja mas cercana al centro de la ventana delante: la
///   hoja entera (hasta `ANCHO_BASE`) si falta o se ve a otro aumento, y
///   muy de cerca, el trozo nitido de lo que se ve si el que hay no lo cubre;
/// - las dos vecinas, si caben en `PRESUPUESTO_MAPAS`;
/// - el texto de la hoja bajo el raton, si se pidio (`elegir`).
///
/// Mientras la rueda cambia el aumento no se pide lo que ya hay pintado (se
/// estira, que es gratis), y **pasando hojas deprisa** no se pide nada: lo
/// que pasa llegaria cuando ya no se ve.
fn lista_de_trabajos(e: &Estado, m: Marco, pasando: bool) -> Vec<(cola::Prioridad, cola::Trabajo)> {
    use cola::{Prioridad, Trabajo};
    let mut lista = Vec::new();
    let n = e.hojas.cuantas();
    if n == 0 {
        return lista;
    }
    let firme = e.zoom_cambiado == 0;
    let s = px(e, m);
    let centro = e.y + m.alto / s / 2.0;
    let vis = visibles(e, m);
    let mut orden: Vec<usize> = vis.clone().collect();
    orden.sort_by(|a, b| {
        let d = |i: usize| (e.hojas.arriba[i] + e.hojas.altos[i] / 2.0 - centro).abs();
        d(*a).total_cmp(&d(*b))
    });
    let deseado = ancho_deseado(e, m);
    let mut usado = 0usize;
    let pedir = |lista: &mut Vec<_>, p, t: Trabajo| {
        if !e.fallidos.contains(&t) {
            lista.push((p, t));
        }
    };
    for i in orden {
        let p = proporcion_de(e, i);
        let base = ancho_para(e, i, m);
        let tiene = e.pintadas.get(&i).map(|(a, _)| *a);
        usado += bytes_de(base.max(tiene.unwrap_or(0)), p);
        let falta = match tiene {
            None => !pasando,
            Some(a) => a != base && firme && !pasando,
        };
        if falta {
            pedir(
                &mut lista,
                Prioridad::Visible,
                Trabajo::Hoja {
                    hoja: i,
                    ancho: base,
                },
            );
        }
        if firme && !pasando && deseado > base {
            let vista_hoja = vista_de_la_hoja(e, i, m);
            if let Some(t) = cola::trozo_para(i, deseado, p, vista_hoja) {
                let ya = e.trozos.get(&i).is_some_and(|(t0, _, _)| {
                    matches!(t0, Trabajo::Trozo { ancho, .. } if *ancho == deseado)
                        && cola::cubre(*t0, p, vista_hoja)
                });
                if !ya {
                    pedir(&mut lista, Prioridad::Visible, t);
                }
                if let Some((_, w, h)) = cola::trozo_en_fracciones(t, p) {
                    usado += w as usize * h as usize * 4;
                }
            }
        }
    }
    if !pasando {
        let antes = vis.start.checked_sub(1);
        let despues = (vis.end < n).then_some(vis.end);
        for i in [despues, antes].into_iter().flatten() {
            let p = proporcion_de(e, i);
            let base = ancho_para(e, i, m);
            let bytes = bytes_de(base, p);
            if usado + bytes > PRESUPUESTO_MAPAS {
                continue;
            }
            usado += bytes;
            let falta = match e.pintadas.get(&i) {
                None => true,
                Some((a, _)) => *a != base && firme,
            };
            if falta {
                pedir(
                    &mut lista,
                    Prioridad::Vecina,
                    Trabajo::Hoja {
                        hoja: i,
                        ancho: base,
                    },
                );
            }
        }
    }
    if e.office.is_none()
        && let Some(i) = e.elegir.falta
        && i < n
        && !e.elegir.hojas.contains_key(&i)
    {
        pedir(&mut lista, Prioridad::Texto, Trabajo::Texto { hoja: i });
    }
    lista
}

/// Pide al hilo lo que falta (ver [`lista_de_trabajos`]) y suelta lo que ya
/// no cabe: de lo pintado, lo que se ve siempre se queda; lo demas, de lo
/// mas cercano a lo mas lejano mientras quepa en `PRESUPUESTO_MAPAS`.
fn pedir_hojas(e: &mut Estado, tx: &mpsc::Sender<Pedido>, m: Marco) {
    if e.hojas.cuantas() == 0 {
        return;
    }
    let ahora = ahora_ms();
    let vis = visibles(e, m);
    if vis.start != e.paso.0 {
        if ahora.saturating_sub(e.paso.1) < MS_PASANDO {
            e.pasando_hasta = ahora + MS_PASANDO;
        }
        e.paso = (vis.start, ahora);
    }
    let pasando = ahora < e.pasando_hasta;
    let lista = lista_de_trabajos(e, m, pasando);
    if lista != e.ultima_lista {
        let _ = tx.send(Pedido::Quiero {
            lista: lista.clone(),
            visto: e.visto,
        });
        e.ultima_lista = lista;
    }
    // Los trozos, solo de lo que se ve y si sigue haciendo falta acercarse.
    let de_cerca = ancho_deseado(e, m) > ANCHO_BASE || e.zoom_cambiado != 0;
    e.trozos.retain(|i, _| de_cerca && vis.contains(i));
    let en_trozos: usize = e
        .trozos
        .values()
        .filter_map(|(t, _, _)| cola::trozo_en_fracciones(*t, proporcion_de(e, t.hoja())))
        .map(|(_, w, h)| w as usize * h as usize * 4)
        .sum();
    let items: Vec<(usize, usize, usize)> = e
        .pintadas
        .iter()
        .map(|(i, (a, _))| (*i, bytes_de(*a, proporcion_de(e, *i)), distancia(&vis, *i)))
        .filter(|(_, _, d)| *d <= HOJAS_DE_RESERVA)
        .collect();
    let quedan = cola::lo_que_cabe(&items, PRESUPUESTO_MAPAS.saturating_sub(en_trozos));
    e.pintadas.retain(|i, _| quedan.contains(i));
    elegir::olvidar_lejos(e, vis);
}

/// Los bytes de los mapas de bits de las hojas y de los trozos.
#[cfg(test)]
fn bytes_en_mapas(e: &Estado) -> usize {
    let hojas: usize = e
        .pintadas
        .iter()
        .map(|(i, (a, _))| bytes_de(*a, proporcion_de(e, *i)))
        .sum();
    let trozos: usize = e
        .trozos
        .values()
        .filter_map(|(t, _, _)| cola::trozo_en_fracciones(*t, proporcion_de(e, t.hoja())))
        .map(|(_, w, h)| w as usize * h as usize * 4)
        .sum();
    hojas + trozos
}

/// Lee del disco lo anotado de las hojas que se ven (una vez cada una).
fn cargar_capas_visibles(e: &mut Estado, ruta: &Path, m: Marco) {
    for i in visibles(e, m) {
        // Una hoja de Office aun sin maquetar no sabe su alto de verdad: su
        // tinta se lee cuando llegue.
        if e.office.as_ref().is_some_and(|o| o.pagina(i).is_none()) {
            continue;
        }
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
        // En oscuro, mas oscuro que la hoja (que es gris muy oscuro): que se
        // vea donde acaba cada una.
        if e.oscuro {
            Color {
                r: 0.055,
                g: 0.055,
                b: 0.063,
                a: 1.0,
            }
        } else {
            FONDO_PDF
        },
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
    // Modo oscuro: el papel (la hoja que aun no llega, los margenes de
    // anotar) y el de la tinta, oscuros; las hojas, al pintarlas.
    let papel = if e.oscuro {
        pixpin_render::oscuro::PAPEL
    } else {
        Color::BLANCO
    };
    let papel_margen = if e.oscuro && !e.fotografiando {
        pixpin_render::oscuro::invertir_luz(PAPEL_DEL_MARGEN)
    } else {
        PAPEL_DEL_MARGEN
    };
    if let Some(o) = e.office.as_ref() {
        o.poner_oscuro(e.oscuro && !e.fotografiando);
    }
    // La hoja de un PDF en oscuro: la matriz de color en la GPU, sin tocar
    // ni duplicar la cache.
    // Fotografiando para imprimir, la hoja con sus colores de verdad.
    let oscuro = e.oscuro && !e.fotografiando;
    let hoja_pdf = |b: &ID2D1Bitmap1, caja: RectF| {
        if oscuro {
            p.bitmap_oscuro(b, caja, None)
        } else {
            p.bitmap(b, caja, None, false)
        }
    };
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
                papel_margen,
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
                papel_margen,
            );
        }
        let hoja = RectF {
            x: 0.0,
            y: 0.0,
            ancho: vista::ANCHO_HOJA,
            alto,
        };
        // Un Office se pinta en vectores, a la transformada de la hoja.
        let en_vectores = e.office.as_mut().is_some_and(|o| o.pintar_hoja(p, i));
        match e.pintadas.get(&i) {
            _ if en_vectores => {}
            Some((_, b)) => hoja_pdf(b, hoja),
            None => {
                p.rellenar(hoja, papel);
                let n = format!("{}", i + 1);
                p.texto(&n, 40.0, 40.0, 48.0, con_alfa(APAGADO, 0.6));
            }
        }
        // Muy de cerca, lo que se ve nitido encima de la hoja (que hace de
        // fondo mientras llega).
        if !en_vectores && let Some((_, caja, b)) = e.trozos.get(&i) {
            hoja_pdf(b, *caja);
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
        // N: la hoja (y sus margenes de anotar) en negativo. Antes de lo
        // encontrado, lo que suena y la tinta, que conservan sus colores.
        if e.en_negativo && !e.fotografiando {
            p.invertir_colores(
                RectF {
                    x: -izq,
                    y: 0.0,
                    ancho: izq + vista::ANCHO_HOJA + der,
                    alto,
                },
                0.0,
            );
        }
        marcas_de_busqueda(e, p, i);
        office::pintar_seleccion(e, p, i);
        elegir::pintar_seleccion(e, p, i);
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
                // En negativo el papel es negro: la tinta se ajusta a el
                // como sobre cualquier papel oscuro y se sigue leyendo.
                lector_tinta::papel(if e.en_negativo && !e.fotografiando {
                    papel.invertido()
                } else {
                    papel
                }),
                activa,
            );
        }
    }
    p.desplazar(0.0, 0.0);

    // Fotografiando un marco para imprimirlo: solo las hojas y su tinta.
    if e.fotografiando {
        e.botones = Vec::new();
        return;
    }
    // Los marcos numerados, pegados al documento (`marcos_doc`).
    let (ex, ey) = (e.x, e.y);
    e.marcos.pintar(
        p,
        0,
        &|q| ((q[0] as f32 - ex) * s, (q[1] as f32 - ey) * s),
        m.e,
        m.alto,
        &crate::marcos_doc::pista(textos),
    );

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
        // Pegado al canto de su hoja SIEMPRE (20-oct-2026, el usuario: «si
        // muevo la hoja se mueve para siempre estar dentro de la vista…
        // tiene que quedarse en el borde de la hoja»): antes se recortaba a
        // la ventana y perseguia a la vista. Fuera de ella, no se ve.
        if sy > -40.0 && sy < m.alto + 40.0 && sx > -40.0 && sx < m.ancho + 40.0 {
            lector::redondel(p, &mm.emoji, (sx, sy), 14.0 * m.e, 0.8);
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
    lector::pintar_riel_con(
        p,
        &riel,
        &emojis,
        e.encima,
        e.poniendo_marca,
        e.anotando,
        e.marcos.activo,
        &textos.t("marca-elige"),
    );
    // Abajo: la barra de escuchar o los mandos de los lados (los mismos del
    // lector de Word: espacio a cada lado y el candado), con la pastilla o
    // con el raton en la franja de abajo.
    if e.voz.is_some() && !e.poniendo_marca {
        for (r, b) in voz::pintar_barra(e, p, m) {
            botones.push((r, Accion::Voz(b)));
        }
    } else if let Some(a) = e.auto.as_ref().filter(|_| !e.poniendo_marca && !e.anotando) {
        // La pagina que sube sola: lo que queda, contando desde la hoja de
        // arriba (`firstVisibleItemIndex` del movil).
        let quedan = match e.palabras {
            None => textos.t("lector-auto-contando"),
            Some(n) => {
                let progreso = (e.y.max(0.0) / e.hojas.total.max(1.0)).clamp(0.0, 1.0);
                lector::rotulo_de_lo_que_queda(textos, n, progreso, a.ppm, lector::minutos_de_hoy())
            }
        };
        let velocidad = lector::rotulo_de_ppm(textos, a.ppm);
        for (r, b) in
            lector::pintar_barra_de_auto(p, m.ancho, m.alto, m.e, &velocidad, a.en_marcha, &quedan)
        {
            botones.push((r, Accion::AutoBoton(b)));
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
        || (!e.en_pin && !e.hallar.caja.abierto && (e.pastilla_hasta > ahora_ms() || e.panel))
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
    let hoja = if let Some(t) = e.renombre.escrito() {
        if t.is_empty() {
            " ".to_string()
        } else {
            t.to_owned()
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
    // En pantalla completa, ademas: luna/sol y volver a pin.
    let iconos_n = if e.en_pin { 3.0 } else { 5.0 };
    let total = w + 28.0 * m.e + iconos_n * lado;
    let caja = RectF {
        x: (m.ancho - total) / 2.0,
        y: 14.0 * m.e,
        ancho: total,
        alto: h + 14.0 * m.e,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(CRISTAL, 0.94));
    // Escribiendo el nombre, la caja de una linea comun (la rayita donde se
    // escribe y lo elegido) con su subrayado; si no, el nombre.
    let x0 = caja.x + 14.0 * m.e;
    if !e.renombre.pintar_escribiendo(p, caja, x0, w, tam, m.e) {
        p.texto(&hoja, x0, caja.y + 7.0 * m.e, tam, TEXTO);
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
        // **Que suba sola** (`KeyboardDoubleArrowDown`, «Desplazar sola»).
        (&lector::DOBLE_FLECHA_ABAJO, e.auto.is_some(), Accion::Auto),
        (&material::MORE_VERT, false, Accion::Engranaje),
        // Luna/sol: el modo oscuro de los visores (en pin, en su barra).
        (
            if e.oscuro {
                &material::LIGHT_MODE
            } else {
                &material::DARK_MODE
            },
            false,
            Accion::Oscuro,
        ),
        // De pantalla completa, de vuelta a pin.
        (&material::FULLSCREEN_EXIT, false, Accion::PantallaCompleta),
    ]
    .into_iter()
    .filter(|(_, _, a)| !matches!(a, Accion::PantallaCompleta | Accion::Oscuro) || !e.en_pin)
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

/// **Lo que va en el «⋯»** (19-oct-2026, el usuario: «muchas son repetidas»):
/// solo lo que NO tiene ya su boton. Escuchar, desplazar sola y guardar el
/// PDF con lo anotado se quedan en la barra, no aqui otra vez.
fn filas_del_panel(e: &Estado, textos: &Catalogo) -> Vec<(String, Accion)> {
    let mut filas = Vec::new();
    // Un Word o un PowerPoint: imprimir, y lo suyo (presentar, o leerlo
    // reflujado).
    if let Some(o) = &e.office {
        if o.tipo == pixpin_office::Tipo::PowerPoint {
            filas.push((textos.t("lector-presentar"), Accion::Presentar));
        } else {
            filas.push((textos.t("lector-modo-lectura"), Accion::ModoLectura));
        }
        filas.push((textos.t("lector-imprimir"), Accion::Imprimir));
        // Compartir: un Word con su texto; lo demas, con sus hojas impresas.
        filas.push((textos.t("compartir-lector"), Accion::Compartir));
    } else {
        // «Ver como texto» va primero, en «Vista», como en el engranaje del movil.
        filas.push((textos.t("lector-ver-como-texto"), Accion::ComoTexto));
        filas.push((textos.t("compartir-lector"), Accion::Compartir));
        filas.push((textos.t("visor-abrir-carpeta"), Accion::AbrirCarpeta));
    }
    if !e.marcas.is_empty() {
        filas.push((textos.t("visor-quitar-marcadores"), Accion::QuitarMarcas));
    }
    if !e.marcos.lista.is_empty() {
        filas.push((textos.t("marcos-imprimir"), Accion::ImprimirMarcos));
        filas.push((textos.t("marcos-borrar"), Accion::BorrarMarcos));
    }
    filas.push((textos.t("lector-quitar-tinta"), Accion::QuitarTinta));
    // Solo si no lo es ya: como en el movil, anotar no crea ningun proyecto.
    if e.office.is_none() && !e.donde.es_de_un_proyecto() {
        filas.push((textos.t("lector-al-proyecto"), Accion::AlProyecto));
    }
    // El apartado «Voz» del movil: las voces de Microsoft y cual.
    filas.extend(
        crate::leer_en_voz::filas(textos, e.ubicacion.raiz())
            .into_iter()
            .map(|(t, a)| (t, Accion::AjusteVoz(a))),
    );
    filas
}

/// **El menu «⋯»**: el desplegable de siempre de la app (el de los pines y
/// las tarjetas), no un panel abajo. Devuelve lo elegido.
fn menu(e: &Estado, textos: &Catalogo) -> Option<Accion> {
    let filas = filas_del_panel(e, textos);
    let entradas: Vec<(u32, String)> = filas
        .iter()
        .enumerate()
        .map(|(i, (t, _))| (i as u32 + 1, t.clone()))
        .collect();
    let hwnd = windows::Win32::Foundation::HWND(e.hwnd as *mut core::ffi::c_void);
    let k = pixpin_shell::menu_llano(hwnd, &entradas)?;
    filas.get(k as usize - 1).map(|(_, a)| *a)
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
        Some(Arrastre::Elegir) if e.office.is_some() => office::mover(e, m),
        Some(Arrastre::Elegir) => elegir::mover(e, m),
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
        DestinoRiel::Imprimir => {
            alternar_marcos(e, ruta);
            return false;
        }
        DestinoRiel::Hojita => {
            // Anotar y poner marcos no van a la vez (como en el plano).
            if e.marcos.activo {
                e.marcos.alternar();
            }
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
    // Office: sobre texto, se elige (o se sigue un hipervinculo).
    if office::pulsar(e, m) {
        e.arrastre = Some(Arrastre::Elegir);
        return true;
    }
    // PDF: sobre texto se elige; en un enlace, se sigue al soltar.
    if elegir::pulsar(e, m) {
        e.arrastre = Some(Arrastre::Elegir);
        return true;
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
        Some(Arrastre::Elegir) if e.office.is_some() => {
            office::soltar(e);
        }
        Some(Arrastre::Elegir) => elegir::soltar(e, m),
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
    // Un PowerPoint pasa de diapositiva en diapositiva.
    if !e.anotando && office::tecla_de_diapositivas(e, vk, shift, ctrl, m) {
        acotar(e, m);
        return true;
    }
    match (vk, ctrl) {
        (VK_ESCAPE, _) => {
            if let Some(o) = e.office.as_mut()
                && o.seleccion.take().is_some()
            {
                // La primera Esc quita lo elegido.
            } else if e.elegir.seleccion.take().is_some() {
                // Lo elegido en un PDF, tambien.
            } else if e.poniendo_marca || e.panel {
                e.poniendo_marca = false;
                e.panel = false;
            } else if e.voz.is_some() {
                // La primera Esc cierra la barra de escuchar.
                voz::dejar(e);
            } else if e.auto.is_some() {
                // Y la de la pagina que sube sola (el `BackHandler` del movil).
                e.auto = None;
            } else if e.anotando {
                alternar_anotar(e, ruta);
            } else if !e.en_pin {
                // En pantalla completa, Esc vuelve a pin; en pin, cierra.
                e.pedir_pantalla_completa = true;
            } else {
                return false;
            }
        }
        // F11, pantalla completa y vuelta, como en cualquier programa.
        (0x7A, _) => e.pedir_pantalla_completa = true,
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
        (VK_D, false) => alternar_auto(e, ruta),
        (VK_G, false) => e.panel = true,
        // N: las hojas en negativo y vuelta, como en los pines.
        (VK_N, false) => e.en_negativo = !e.en_negativo,
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
// La pagina que sube sola (`LectorPdfActivity`, v0.92) y «Ver como texto»

/// Abre o cierra la barra de la pagina que sube sola. Las palabras salen
/// del texto de las hojas, que se pide a su hilo si aun no estaba.
fn alternar_auto(e: &mut Estado, ruta: &Path) {
    if e.auto.take().is_some() || e.hojas.cuantas() == 0 {
        return;
    }
    voz::dejar(e);
    voz::pedir_el_texto(e, ruta);
    e.auto = Some(pixpin_docs::auto_desplazar::Marcha::nueva(
        lector::ppm_guardadas(&e.ubicacion),
    ));
}

/// **Las palabras del documento** (`PdfAHtml.contarPalabras`): las del
/// texto de sus hojas; un escaneo (o un PDF que no se lee) cuenta
/// `POR_HOJA_SI_NO_HAY` por hoja, lo de una pagina corriente. Nada mientras
/// el texto no ha llegado.
fn palabras_de(texto: Option<&TextoPdf>, hojas: usize) -> Option<usize> {
    let cuenta = match texto? {
        Ok(paginas) => paginas
            .iter()
            .map(|p| pixpin_docs::auto_desplazar::contar_palabras(&p.texto))
            .sum(),
        Err(_) => 0,
    };
    Some(if cuenta > 0 {
        cuenta
    } else {
        hojas * pixpin_docs::pdf_a_texto::POR_HOJA_SI_NO_HAY
    })
}

/// Si la pagina esta subiendo ahora (para despertar a sesenta por segundo).
fn subiendo(e: &Estado) -> bool {
    e.auto.as_ref().is_some_and(|a| a.en_marcha) && !e.anotando && e.voz.is_none()
}

/// **Una vuelta del bucle**: baja lo que toque, en pixeles enteros de
/// pantalla; la velocidad sale del alto de todas las hojas y de sus
/// palabras. La rueda puede mover la vista entre medias; al final se para.
fn subir_sola(e: &mut Estado, m: Marco) -> bool {
    if e.auto.is_none() {
        return false;
    }
    let mut cambio = false;
    if e.palabras.is_none() {
        e.palabras = palabras_de(e.hallar.texto.as_ref(), e.hojas.cuantas());
        cambio = e.palabras.is_some();
    }
    let quieta = e.anotando || e.voz.is_some() || e.palabras.is_none();
    let s = px(e, m);
    let (palabras, alto) = (e.palabras.unwrap_or(0), e.hojas.total);
    let Some(a) = e.auto.as_mut() else {
        return cambio;
    };
    let v = pixpin_docs::auto_desplazar::pixeles_por_segundo(a.ppm, palabras, alto) * s;
    let n = a.paso(ahora_ms(), v, quieta);
    if n <= 0 {
        return cambio;
    }
    let antes = e.y;
    e.y += n as f32 / s;
    acotar(e, m);
    if e.y <= antes + 1e-4
        && let Some(a) = e.auto.as_mut()
    {
        a.al_final();
    }
    true
}

/// Un boton de la barra; la velocidad nueva se recuerda en este equipo.
fn boton_de_auto(e: &mut Estado, b: lector::BotonAuto) {
    let Some(a) = e.auto.as_mut() else {
        return;
    };
    match b {
        lector::BotonAuto::Menos | lector::BotonAuto::Mas => {
            a.cambiar(if b == lector::BotonAuto::Mas { 1 } else { -1 });
            lector::guardar_ppm(&e.ubicacion, a.ppm);
        }
        lector::BotonAuto::Alternar => a.alternar(),
        lector::BotonAuto::Cerrar => e.auto = None,
    }
}

/// **«Ver como texto»** (`VisorHtmlActivity.abrirPdfComoTexto`): el PDF se
/// pasa a texto en un hilo (la segunda vez sale de la cache) y se abre en el
/// lector de Word. Un escaneo o un PDF cifrado lo dicen aqui.
fn ver_como_texto(e: &mut Estado, textos: &Catalogo, ruta: &Path) {
    if e.como_texto.is_some() || e.office.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let raiz = e.ubicacion.raiz().to_path_buf();
    let pdf = ruta.to_path_buf();
    let del_aparato = crate::leer_en_voz::idioma_de_la_app(textos);
    let hoja = textos.t("lector-hoja");
    let lanzado = std::thread::Builder::new()
        .name("pdf-como-texto".into())
        .spawn(move || {
            let _ = tx.send(crate::visor::como_texto::preparar(
                &raiz,
                &pdf,
                &del_aparato,
                &hoja,
            ));
        });
    match lanzado {
        Ok(_) => {
            e.como_texto = Some(rx);
            e.aviso = Some((textos.t("lector-pasando-a-texto"), ahora_ms() + 3000));
        }
        Err(err) => tracing::warn!(?err, "sin hilo para pasar el PDF a texto"),
    }
}

/// Lo que dice el hilo de «Ver como texto»: abrir la pagina o el aviso.
fn recoger_como_texto(e: &mut Estado, textos: &Catalogo, ubicacion: &Ubicacion) -> bool {
    let Some(rx) = &e.como_texto else {
        return false;
    };
    let hecho = match rx.try_recv() {
        Ok(h) => h,
        Err(mpsc::TryRecvError::Empty) => return false,
        Err(mpsc::TryRecvError::Disconnected) => Err(crate::visor::como_texto::NoSale::NoSeLee),
    };
    e.como_texto = None;
    match hecho {
        Ok(pagina) => {
            e.aviso = None;
            crate::visor::lanzar(
                crate::compartir::idioma_de(ubicacion),
                ubicacion.clone(),
                &pagina,
            );
        }
        Err(n) => e.aviso = Some((textos.t(n.clave()), ahora_ms() + 5000)),
    }
    true
}

// ---------------------------------------------------------------------------
// Hacer

/// **El boton del marco o la I**: entrar o salir de poner los marcos
/// numerados. Como en el plano, poner marcos deja de anotar y de marcar.
fn alternar_marcos(e: &mut Estado, ruta: &Path) {
    e.marcos.alternar();
    if e.marcos.activo {
        e.poniendo_marca = false;
        if e.anotando {
            alternar_anotar(e, ruta);
        }
    }
}

/// Si en `q` (pixeles de la ventana) hay interfaz: un boton o el riel.
fn sobre_la_interfaz(e: &Estado, m: Marco, q: (f32, f32)) -> bool {
    riel_de(e, m).contiene(lector::punto(q.0, q.1))
        || e.botones.iter().any(|(r, _)| lector::dentro(r, q.0, q.1))
}

/// El documento bajo el raton (unidades del lector: x de la hoja, y de
/// todas las hojas seguidas).
/// **A ese marco**: acercarse hasta que llena la ventana, centrado (lo
/// mismo que para su foto, sin fotografiar).
fn ir_al_marco(e: &mut Estado, m: Marco, i: usize) {
    let Some(marco) = e.marcos.lista.get(i).cloned() else {
        return;
    };
    let s = crate::marcos_doc::px_para_llenar(&marco, m.ancho, m.alto) as f32;
    e.zoom = zoom_para(e, m, s);
    let s = px(e, m);
    let c = marco.centro();
    e.x = c[0] as f32 - m.ancho / (2.0 * s);
    e.y = c[1] as f32 - m.alto / (2.0 * s);
}

fn doc_del_raton(e: &Estado, m: Marco) -> [f64; 2] {
    let s = px(e, m);
    [(e.x + e.raton.0 / s) as f64, (e.y + e.raton.1 / s) as f64]
}

/// Las fotos de los marcos para imprimirlos en grupo (`marcos_doc`): por
/// cual va, cuando se puso a su vista y lo de antes para volver.
pub(crate) struct FotosDeMarcos {
    pub(crate) i: usize,
    pub(crate) desde: u64,
    pub(crate) antes: (f32, f32, f32),
    fotos: Vec<ImagenRgba>,
    titulo: String,
}

impl FotosDeMarcos {
    /// Para empezar desde el primer marco, con la vista `antes` para volver.
    pub(crate) fn nuevas(antes: (f32, f32, f32), titulo: &str) -> FotosDeMarcos {
        FotosDeMarcos {
            i: 0,
            desde: 0,
            antes,
            fotos: Vec::new(),
            titulo: titulo.to_string(),
        }
    }

    /// Lo pintado ya esta en la pantalla: la foto de `r` (pixeles de la
    /// ventana `hwnd`) y al siguiente marco.
    pub(crate) fn fotografiar(&mut self, hwnd: isize, r: RectF) {
        pixpin_shell::overlay::esperar_composicion();
        if let Some((_, cliente)) = pixpin_shell::overlay::area_cliente(Some(hwnd)) {
            let area = pixpin_geom::Rect {
                x: cliente.x + r.x.round() as i32,
                y: cliente.y + r.y.round() as i32,
                ancho: r.ancho.round().max(1.0) as u32,
                alto: r.alto.round().max(1.0) as u32,
            };
            match area
                .interseccion(cliente)
                .and_then(crate::marco_imprimir::foto_de_pantalla)
            {
                Some(foto) => self.fotos.push(foto),
                None => tracing::warn!(i = self.i, "marcos: sin foto del marco"),
            }
        }
        self.i += 1;
        self.desde = 0;
    }

    /// Todas juntas al dialogo de imprimir, una hoja por marco.
    pub(crate) fn imprimir(self, hwnd: isize) {
        tracing::info!(marcos = self.fotos.len(), "marcos: a imprimir en grupo");
        if let Err(err) = crate::marco_imprimir::imprimir_fotos(hwnd, self.fotos, &self.titulo) {
            tracing::warn!(?err, "marcos: no se pudo imprimir");
        }
    }
}

/// Ctrl+P con marcos: empieza a fotografiarlos (anotando, primero se deja
/// de anotar, para que lo anotado salga sin la barra). Sin marcos, `false`.
fn imprimir_marcos(e: &mut Estado, ruta: &Path) -> bool {
    if e.marcos.lista.is_empty() {
        return false;
    }
    if e.anotando {
        alternar_anotar(e, ruta);
    }
    e.poniendo_marca = false;
    e.fotos = Some(FotosDeMarcos::nuevas((e.x, e.y, e.zoom), &e.nombre));
    true
}

/// **Un paso de las fotos de los marcos**: se acerca al siguiente hasta
/// que llena la ventana, deja que lleguen las hojas nitidas, lo fotografia
/// y, con el ultimo, vuelve a como estaba y abre el dialogo de imprimir con
/// todas (una hoja por marco). Devuelve si hay que pintar.
fn paso_de_las_fotos(e: &mut Estado, m: Marco) -> bool {
    let Some(mut f) = e.fotos.take() else {
        return false;
    };
    let ahora = ahora_ms();
    let Some(marco) = e.marcos.lista.get(f.i).cloned() else {
        // Se quitaron marcos mientras tanto: lo hecho, a imprimir.
        terminar_fotos(e, f);
        return true;
    };
    if f.desde == 0 {
        let base = ancho_base(m) / vista::ANCHO_HOJA;
        let s = crate::marcos_doc::px_para_llenar(&marco, m.ancho, m.alto) as f32;
        e.zoom = (s / base).clamp(0.05, vista::ZOOM_MAXIMO_PDF);
        let s = px(e, m);
        let c = marco.centro();
        e.x = c[0] as f32 - m.ancho / (2.0 * s);
        e.y = c[1] as f32 - m.alto / (2.0 * s);
        // Las hojas nitidas ya, sin esperar a que el zoom se quede quieto.
        e.zoom_cambiado = 0;
        e.fotografiando = true;
        f.desde = ahora;
        e.fotos = Some(f);
        return true;
    }
    // Un Word se pinta en vectores al momento; un PDF espera sus hojas.
    let espera = if e.office.is_some() { 250 } else { 900 };
    if ahora < f.desde + espera {
        e.fotos = Some(f);
        return false;
    }
    let s = px(e, m);
    let (ex, ey) = (e.x, e.y);
    let r = crate::marcos_doc::caja_en_pantalla(marco.caja, &|q| {
        ((q[0] as f32 - ex) * s, (q[1] as f32 - ey) * s)
    });
    f.fotografiar(e.hwnd, r);
    if f.i >= e.marcos.lista.len() {
        terminar_fotos(e, f);
    } else {
        e.fotos = Some(f);
    }
    true
}

fn terminar_fotos(e: &mut Estado, f: FotosDeMarcos) {
    (e.x, e.y, e.zoom) = f.antes;
    e.fotografiando = false;
    e.zoom_cambiado = ahora_ms();
    f.imprimir(e.hwnd);
}

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
    let antes: Vec<i64> = e.marcas.iter().map(|m| m.id).collect();
    e.marcas = marcas::con(&e.marcas, x, y, emoji, ahora_ms() as i64);
    // Y la vista de ese momento: volver a ella es volver con el mismo zoom.
    if let Some(nueva) = e.marcas.iter().find(|m| !antes.contains(&m.id)) {
        e.vistas.poner(
            &nueva.id.to_string(),
            crate::marcos_doc::zoom_a_texto(e.zoom as f64, e.x as f64, e.y as f64),
        );
    }
    e.poniendo_marca = false;
    guardar_ajustes(e, ruta);
}

/// A esa hoja y a esa altura, con la marca **arriba**: es lo que se fue a
/// ver.
fn ir_a_la_marca(e: &mut Estado, i: usize) {
    if let Some(mm) = e.marcas.get(i) {
        // Con su vista guardada, a ella entera (zoom y sitio exacto); las de
        // antes, solo a su altura.
        if let Some((z, x, y)) = e
            .vistas
            .de_marca(&mm.id.to_string())
            .and_then(crate::marcos_doc::zoom_de_texto)
        {
            e.zoom = z as f32;
            e.x = x as f32;
            e.y = y as f32;
            e.zoom_cambiado = ahora_ms();
        } else {
            e.y = e.hojas.y_de(mm.y);
        }
    }
}

/// Las hojas cuyo ultimo paso (o el recien deshecho, `deshecho`) es el
/// turno del asistente que tenga la hoja `i` en esa misma situacion.
fn hojas_del_turno(e: &Estado, i: usize, deshecho: bool) -> Vec<usize> {
    let esta = |c: &crate::lector_tinta::Capa| {
        c.turno
            .as_ref()
            .filter(|t| {
                if deshecho {
                    t.deshecho(&c.escena)
                } else {
                    t.arriba(&c.escena)
                }
            })
            .map(|t| t.grupo.clone())
    };
    let Some(grupo) = e.capas.get(&i).and_then(esta) else {
        return Vec::new();
    };
    let mut v: Vec<usize> = e
        .capas
        .iter()
        .filter(|(_, c)| esta(c).as_deref() == Some(grupo.as_str()))
        .map(|(k, _)| *k)
        .collect();
    v.sort_unstable();
    v
}

/// Quita de `pila` la ultima vez que sale cada hoja de `hojas`.
fn quitar_ultimas(pila: &mut Vec<usize>, hojas: &[usize]) {
    for h in hojas {
        if let Some(k) = pila.iter().rposition(|x| x == h) {
            pila.remove(k);
        }
    }
}

fn deshacer(e: &mut Estado) {
    while let Some(i) = e.hechos.pop() {
        // Lo que puso el asistente en un turno se quita entero, en todas las
        // hojas que toco, con un solo Ctrl+Z (`lector_tinta::turnos`).
        let turno = hojas_del_turno(e, i, false);
        if !turno.is_empty() {
            let otras: Vec<usize> = turno.iter().copied().filter(|k| *k != i).collect();
            quitar_ultimas(&mut e.hechos, &otras);
            for k in turno {
                if e.capas.get_mut(&k).is_some_and(|c| c.deshacer()) {
                    e.deshechos.push(k);
                }
            }
            e.ultimo_trazo = ahora_ms();
            return;
        }
        if e.capas.get_mut(&i).is_some_and(|c| c.deshacer()) {
            e.deshechos.push(i);
            e.ultimo_trazo = ahora_ms();
            return;
        }
    }
}

fn rehacer(e: &mut Estado) {
    while let Some(i) = e.deshechos.pop() {
        let turno = hojas_del_turno(e, i, true);
        if !turno.is_empty() {
            let otras: Vec<usize> = turno.iter().copied().filter(|k| *k != i).collect();
            quitar_ultimas(&mut e.deshechos, &otras);
            for k in turno {
                if e.capas.get_mut(&k).is_some_and(|c| c.rehacer()) {
                    e.hechos.push(k);
                }
            }
            e.ultimo_trazo = ahora_ms();
            return;
        }
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
        Accion::Imprimir => {
            if let Some(o) = e.office.as_mut() {
                o.pedir_imprimir = true;
            }
            e.panel = false;
        }
        Accion::Presentar => {
            e.panel = false;
            office::presentar(e, ubicacion, ruta, m);
        }
        Accion::ModoLectura => {
            e.panel = false;
            office::modo_lectura(e, ubicacion, ruta);
        }
        // Pulsar el nombre ya no lo cambia (19-oct-2026, el usuario: «puede
        // pasar que se cambie por accidente»).
        Accion::Renombrar => {}
        // Para toda la app; el lector se repinta en la vuelta (`lector_pin`).
        Accion::ImprimirMarcos => {
            imprimir_marcos(e, ruta);
        }
        Accion::BorrarMarcos => e.marcos.borrar(),
        Accion::Oscuro => {
            crate::oscuro::alternar();
        }
        Accion::PantallaCompleta => e.pedir_pantalla_completa = true,
        Accion::AlProyecto => al_proyecto(e, textos, ruta, ubicacion),
        Accion::Buscar(b) => {
            let h = e.hallar.caja.boton(b);
            hecho_de_buscar(e, h, ruta);
        }
        Accion::Engranaje => {
            e.panel = false;
            if let Some(a) = menu(e, textos) {
                hacer(e, a, textos, ruta, ubicacion, tx, m);
            }
        }
        Accion::Escuchar => {
            // Escuchar cierra la pagina que sube sola (`dejarDeAutoDesplazar`).
            e.auto = None;
            voz::escuchar(e, textos, ruta, m)
        }
        Accion::Voz(b) => voz::boton(e, b, ruta, m),
        Accion::AjusteVoz(crate::leer_en_voz::AjusteVoz::Microsoft) => {
            crate::leer_en_voz::alternar_microsoft(e.ubicacion.raiz());
        }
        // El menu necesita la ventana: lo saca el bucle.
        Accion::AjusteVoz(crate::leer_en_voz::AjusteVoz::ElegirVoz) => e.elegir_voz = true,
        Accion::Auto => {
            e.panel = false;
            alternar_auto(e, ruta);
        }
        Accion::AutoBoton(b) => boton_de_auto(e, b),
        Accion::ComoTexto => {
            e.panel = false;
            ver_como_texto(e, textos, ruta);
        }
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
            // Un PowerPoint (o lo que no es un Word): sus hojas impresas, con
            // el «Exportar» comun.
            let cosa = match e.office.as_ref().map(|o| o.tipo) {
                Some(t) if t != pixpin_office::Tipo::Word => {
                    office::exportable(e, ruta).map(crate::compartir::Cosa::Visor)
                }
                _ => None,
            }
            .unwrap_or_else(|| crate::compartir::Cosa::Documento(ruta.to_path_buf()));
            crate::compartir::ventana::abrir(
                crate::compartir::idioma_de(ubicacion),
                ubicacion.clone(),
                cosa,
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
            e.al_dia.olvidar();
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

/// Solo lo que cambio: pedir donde escribir le pone su dibujo a la hoja del
/// proyecto, y mirar una hoja no la anota. Lo que cambio tambien en el
/// disco con el lector abierto se junta, no se pisa (`al_dia`).
fn guardar_capas(e: &mut Estado, ruta: &Path) {
    al_dia::guardar(e, ruta);
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
    match e.renombre.tecla(vk, shift, ctrl) {
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

/// Los botones propios del lector de PDF en la barra de su pin: lo que iba
/// en la pastilla de arriba (escuchar y los ajustes).
fn botones_del_pin(e: &Estado) -> Vec<crate::lector_pin::BotonLector> {
    use crate::lector_pin::BotonLector;
    let mut v = vec![
        BotonLector {
            id: 1,
            icono: &material::RECORD_VOICE_OVER,
            encendido: e.voz.is_some(),
        },
        BotonLector {
            id: 4,
            icono: &lector::DOBLE_FLECHA_ABAJO,
            encendido: e.auto.is_some(),
        },
        BotonLector {
            id: 2,
            icono: &material::MORE_VERT,
            encendido: false,
        },
    ];
    // Un PowerPoint: presentar, a mano en la barra.
    if e.office
        .as_ref()
        .is_some_and(|o| o.tipo == pixpin_office::Tipo::PowerPoint)
    {
        v.insert(
            0,
            BotonLector {
                id: 3,
                icono: &material::PLAY_ARROW,
                encendido: false,
            },
        );
    }
    v
}

fn accion_del_pin(id: u8) -> Option<Accion> {
    Some(match id {
        1 => Accion::Escuchar,
        2 => Accion::Engranaje,
        3 => Accion::Presentar,
        4 => Accion::Auto,
        _ => return None,
    })
}

/// El nombre que lleva la barra del pin, con la hoja que se ve: «Tema 3 · 4/20».
fn nombre_del_pin(e: &Estado) -> String {
    if e.hojas.cuantas() > 0 {
        format!(
            "{} · {}/{}",
            e.nombre,
            e.hojas.en(e.y.max(0.0)) + 1,
            e.hojas.cuantas()
        )
    } else {
        e.nombre.clone()
    }
}

/// El zoom que da `s` pixeles por unidad del lector.
fn zoom_para(_e: &Estado, m: Marco, s: f32) -> f32 {
    (s / (ancho_base(m) / vista::ANCHO_HOJA)).clamp(0.05, vista::ZOOM_MAXIMO_PDF)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    pub(super) fn marco() -> Marco {
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

    pub(super) fn estado(paginas: usize) -> Estado {
        let medidas = vec![(595.0, 842.0); paginas];
        Estado {
            nombre: "plano".into(),
            ajustes: lectura::Ajustes::default(),
            hojas: vista::Hojas::colocar(&medidas),
            medidas,
            pintadas: HashMap::new(),
            ultima_lista: Vec::new(),
            trozos: HashMap::new(),
            visto: 0,
            fallidos: Default::default(),
            paso: (0, 0),
            pasando_hasta: 0,
            elegir: Default::default(),
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            zoom_cambiado: 0,
            marcas: Vec::new(),
            capas: HashMap::new(),
            al_dia: Default::default(),
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
            hwnd: 0,
            marcos: Default::default(),
            vistas: Default::default(),
            fotos: None,
            fotografiando: false,
            panel: false,
            poniendo_marca: false,
            en_negativo: false,
            oscuro: false,
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
            elegir_voz: false,
            engancharse: false,
            en_pin: false,
            pedir_pantalla_completa: false,
            office: None,
            auto: None,
            palabras: None,
            ubicacion: Ubicacion::Portable {
                raiz: std::env::temp_dir()
                    .join(format!("pixpin-lector-pdf-prueba-{}", std::process::id())),
            },
            como_texto: None,
        }
    }

    // ---- La pagina que sube sola y «Ver como texto» ----

    #[test]
    fn la_pagina_sube_sola_hasta_el_final_y_se_para() {
        let mut e = estado(3);
        let m = marco();
        acotar(&mut e, m);
        e.auto = Some(pixpin_docs::auto_desplazar::Marcha::nueva(700));
        // Sin texto aun: no corre, la barra dice «Contando…».
        assert!(!subir_sola(&mut e, m));
        e.hallar.texto = Some(Ok(vec![pagina_con("tres palabras aqui", 0.1); 3]));
        // Al llegar el texto se cuentan: 9 palabras, una velocidad enorme.
        subir_sola(&mut e, m);
        assert_eq!(e.palabras, Some(9));
        let y0 = e.y;
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(subir_sola(&mut e, m), "baja");
        assert!(e.y > y0);
        // Hasta el final, y ahi se para sola.
        for _ in 0..400 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            subir_sola(&mut e, m);
            if !e.auto.as_ref().unwrap().en_marcha {
                break;
            }
        }
        assert!(!e.auto.as_ref().unwrap().en_marcha, "al final se para");
        // Anotando no corre (caso negativo).
        let mut a = estado(3);
        acotar(&mut a, m);
        a.auto = Some(pixpin_docs::auto_desplazar::Marcha::nueva(700));
        a.palabras = Some(9);
        a.anotando = true;
        let y = a.y;
        subir_sola(&mut a, m);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(!subir_sola(&mut a, m));
        assert_eq!(a.y, y);
    }

    #[test]
    fn un_escaneo_cuenta_lo_de_una_pagina_corriente_por_hoja() {
        let vacias = Ok(vec![pagina_con("", 0.1); 4]);
        assert_eq!(
            palabras_de(Some(&vacias), 4),
            Some(4 * pixpin_docs::pdf_a_texto::POR_HOJA_SI_NO_HAY)
        );
        let cifrado = Err(pixpin_pdf::texto::SinTexto::Cifrado);
        assert_eq!(palabras_de(Some(&cifrado), 2), Some(500));
        assert_eq!(palabras_de(None, 2), None);
        let con = Ok(vec![pagina_con("uno dos", 0.1), pagina_con("tres", 0.1)]);
        assert_eq!(palabras_de(Some(&con), 2), Some(3));
    }

    #[test]
    fn ver_como_texto_se_ofrece_en_un_pdf_y_un_escaneo_lo_dice() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut e = estado(1);
        assert!(
            filas_del_panel(&e, &textos)
                .iter()
                .any(|(_, a)| *a == Accion::ComoTexto)
        );
        // Lo que ya tiene su boton en la barra no se repite en el «⋮».
        for repetida in [Accion::Auto, Accion::Escuchar, Accion::Exportar] {
            assert!(
                !filas_del_panel(&e, &textos)
                    .iter()
                    .any(|(_, a)| *a == repetida),
                "{repetida:?}"
            );
        }
        // Un PDF que es una foto: el hilo contesta y el lector lo avisa.
        let dir =
            std::env::temp_dir().join(format!("pixpin-como-texto-lector-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let foto = dir.join("foto.pdf");
        std::fs::write(&foto, b"%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n").unwrap();
        e.ubicacion = Ubicacion::Portable {
            raiz: dir.join("raiz"),
        };
        ver_como_texto(&mut e, &textos, &foto);
        assert_eq!(
            e.aviso.as_ref().map(|a| a.0.clone()),
            Some(textos.t("lector-pasando-a-texto"))
        );
        let u = e.ubicacion.clone();
        let mut llego = false;
        for _ in 0..200 {
            if recoger_como_texto(&mut e, &textos, &u) {
                llego = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(llego);
        assert_eq!(
            e.aviso.as_ref().map(|a| a.0.clone()),
            Some(textos.t("lector-texto-escaneo"))
        );
        assert!(e.como_texto.is_none());
        let _ = std::fs::remove_dir_all(&dir);
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
    fn los_word_y_powerpoint_de_ahora_van_al_lector_con_sus_paginas() {
        for si in [
            "tesis.docx",
            "a.DOCM",
            "plantilla.dotx",
            "b.dotm",
            "charla.pptx",
            "c.pptm",
            "d.ppsx",
            "e.potx",
        ] {
            assert!(se_abre_office(si), "{si}");
            // Tocar la burbuja o abrir con PixPin los lleva a su lector.
            assert!(crate::lector::se_lee_al_tocar(si), "{si}");
            assert!(crate::lector::tiene_lector(si), "{si}");
        }
        // Caso negativo: los de antes (no son un ZIP), un PDF y una hoja de
        // calculo van por su camino.
        for no in ["viejo.doc", "viejo.ppt", "a.pdf", "b.xlsx", "docx"] {
            assert!(!se_abre_office(no), "{no}");
        }
    }

    #[test]
    fn en_un_powerpoint_avpag_lleva_a_la_siguiente_diapositiva_centrada() {
        let mut e = estado(3);
        // Diapositivas apaisadas: caben enteras en la ventana.
        e.medidas = vec![(960.0, 540.0); 3];
        e.hojas = vista::Hojas::colocar(&e.medidas);
        let m = marco();
        office::ir_a(&mut e, 0, m);
        assert_eq!(office::la_de_ahora(&e, m), 0);
        office::ir_a(&mut e, 2, m);
        assert_eq!(office::la_de_ahora(&e, m), 2);
        // Centrada: tanto aire arriba como abajo.
        let s = px(&e, m);
        let arriba = (e.hojas.arriba[2] - e.y) * s;
        let abajo = m.alto - arriba - e.hojas.altos[2] * s;
        assert!((arriba - abajo).abs() < 1.0, "{arriba} {abajo}");
        // Caso negativo: sin documento de PowerPoint las teclas no son suyas.
        assert!(!office::tecla_de_diapositivas(
            &mut e, VK_NEXT, false, false, m
        ));
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
        let Ok(Pedido::Quiero { lista, .. }) = rx.try_recv() else {
            panic!("se esperaba una lista");
        };
        assert!(!lista.is_empty());
        assert_eq!(lista[0].1.hoja(), 0, "la que se ve primero");
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

    /// **Una hoja de PDF en modo oscuro** (luna/sol), pintada fuera de
    /// pantalla a PNG para mirarla: la misma hoja en claro y en oscuro. La
    /// hoja oscura sale de la matriz de color al pintar: el bitmap guardado
    /// es el mismo. `PIXPIN_PDF` es el PDF (una copia, nunca el original) y
    /// `PIXPIN_MUESTRAS` la carpeta de los PNG:
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lector_de_pdf_en_oscuro -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y un PDF de muestra"]
    fn muestra_del_lector_de_pdf_en_oscuro() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let Some(ruta) = std::env::var_os("PIXPIN_PDF").map(std::path::PathBuf::from) else {
            println!("sin PIXPIN_PDF");
            return;
        };
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let m = Marco {
            ancho: 1200.0,
            alto: 900.0,
            e: 1.0,
            escala_por_cien: 100,
            area: pixpin_geom::Rect {
                x: 0,
                y: 0,
                ancho: 1200,
                alto: 900,
            },
        };
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1200, 900).expect("superficie");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let documento = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
        let mut e = estado(0);
        e.medidas = documento.medidas();
        e.hojas = vista::Hojas::colocar(&e.medidas);
        let hoja: usize = std::env::var("PIXPIN_PDF_HOJA")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
            .min(e.hojas.cuantas() - 1);
        e.zoom = 0.5;
        acotar(&mut e, m);
        e.y = e.hojas.arriba[hoja];
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
        let mut medias = Vec::new();
        for (oscuro, nombre) in [
            (false, "lector-pdf-claro.png"),
            (true, "lector-pdf-oscuro.png"),
        ] {
            e.oscuro = oscuro;
            motor
                .dibujar(&fuera.destino, |p| pintar(&mut e, p, m, &textos))
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            // La luz media del centro de la ventana (la hoja).
            let mut suma = 0u64;
            let mut n = 0u64;
            for y in (alto / 4)..(alto * 3 / 4) {
                for x in (ancho / 4)..(ancho * 3 / 4) {
                    let k = ((y * ancho + x) * 4) as usize;
                    suma += (pixeles[k] as u64 + pixeles[k + 1] as u64 + pixeles[k + 2] as u64) / 3;
                    n += 1;
                }
            }
            medias.push(suma / n.max(1));
            let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(nombre);
            std::fs::write(&salida, png).expect("guardar");
            println!("{}", salida.display());
        }
        println!(
            "luz media de la hoja: claro {} / oscuro {}",
            medias[0], medias[1]
        );
        assert!(medias[0] > 160, "en claro la hoja es clara");
        assert!(medias[1] < 90, "en oscuro la hoja es oscura");
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
