//! **Las hojas del chat se abren como un pin** (el usuario, 11-oct-2026:
//! «que todo siempre se abra en version de pin pero en modo visualizacion, y
//! si tenemos algo como el editor de Excel, se pase a este con un boton
//! dentro de la misma interfaz del pin»).
//!
//! Un Excel adjunto (`.xlsx`, `.xlsm`, `.ods`, `.csv`, `.tsv`) o una tabla
//! creada en el chat ya no se abren en la hoja pintada dentro del historial:
//! se abren en el pin del visor de hojas (`visor_hoja`), en modo ver, con
//! «Editar» en su barra. Editar convierte ESA MISMA ventana en la hoja
//! editable de aqui; «Ver» la devuelve al visor.
//!
//! - **Ver**: un `.xlsx` sin tocar se ve tal cual es (el original, con sus
//!   estilos, como desde el Explorador). Un `.csv`/`.ods`, una tabla del
//!   chat o un libro ya editado se ven con sus tablas pasadas a un `.xlsx`
//!   en memoria (`pixpin_proyecto::tabla_xlsx`), con el mismo pintor.
//! - **Editar** se monta solo al pulsarlo: las tablas del libro se leen la
//!   primera vez (`importar_hojas`) y no se releen al volver a editar.
//! - **Se guarda en el chat**, en el mensaje-tabla de cada hoja
//!   (`libro-<mensaje>-<hoja>`, como el movil), asi la sincronizacion con
//!   Android sigue igual. El archivo original NO cambia, y se dice la
//!   primera vez. Un libro que solo se mira no deja tablas en el chat.
//! - Deshacer y rehacer (Ctrl+Z, Ctrl+Y) con una pila de cambios de celda
//!   (`editor.rs`).

pub mod editor;
pub mod libro;
#[cfg(test)]
mod pruebas;

/// **Mandar al pin desde una prueba** (solo en las pruebas): sin teclas ni
/// clics simulados, la prueba deja una orden y el bucle de la ventana la
/// recoge en su siguiente vuelta, y apunta donde esta la ventana para
/// fotografiarla.
#[cfg(test)]
pub(crate) mod prueba {
    use std::sync::Mutex;

    pub const EDITAR: u8 = 1;
    pub const VER: u8 = 2;
    pub const CERRAR: u8 = 3;
    /// Escribir algo de muestra en la hoja (modo editar).
    pub const ESCRIBIR: u8 = 4;

    static ORDENES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static VENTANA: Mutex<Option<(isize, pixpin_geom::Rect, bool)>> = Mutex::new(None);

    /// Lo llama el bucle en cada vuelta: apunta la ventana y da la orden
    /// que haya.
    pub fn latido(hwnd: isize, area: pixpin_geom::Rect, editando: bool) -> Option<u8> {
        *VENTANA.lock().unwrap_or_else(|e| e.into_inner()) = Some((hwnd, area, editando));
        let mut o = ORDENES.lock().unwrap_or_else(|e| e.into_inner());
        (!o.is_empty()).then(|| o.remove(0))
    }

    pub fn ordenar(orden: u8) {
        ORDENES.lock().unwrap_or_else(|e| e.into_inner()).push(orden);
        if let Some((h, ..)) = ventana() {
            pixpin_shell::overlay::despertar(h);
        }
    }

    /// La ventana abierta: (hwnd, donde, si esta editando).
    pub fn ventana() -> Option<(isize, pixpin_geom::Rect, bool)> {
        *VENTANA.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn olvidar() {
        *VENTANA.lock().unwrap_or_else(|e| e.into_inner()) = None;
        ORDENES.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixpin_hojas::{Libro, Regional, nombre_columna};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::tabla::{Ref, Tabla};
use pixpin_render::icono::material;
use pixpin_render::letras::Letra;
use pixpin_render::{Color, MotorRender, Pintor, RectF};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin};
use pixpin_store::{Catalogo, Ubicacion};

use crate::lector_pin::{AccionPin, BotonLector};
use crate::visor_hoja::Marco;
use editor::{Alineacion, Editor, HojaEditable};

// ---------------------------------------------------------------------------
// De donde viene la hoja

/// Que se abre al tocar una burbuja del chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    /// Un libro adjunto (Excel, OpenDocument, CSV).
    Libro,
    /// Una tabla creada en el chat (la mini-app `tabla`).
    Tabla,
}

/// **Si una burbuja se abre como pin de hoja**, y como. Un `.xls` antiguo no:
/// no se lee, y sigue su camino de siempre (su programa).
pub fn que_abrir(m: &Mensaje) -> Option<Que> {
    if m.clase == Some(Clase::MiniApp) && m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP) {
        return Some(Que::Tabla);
    }
    (libro::es_libro(m) && libro::extension(m) != "xls").then_some(Que::Libro)
}

/// De donde salio la hoja que se ve.
pub(crate) enum Origen {
    /// Un libro adjunto: su mensaje, el fichero y las tablas que ya salieron
    /// de el (en el orden de sus hojas; vacio si nunca se edito).
    Libro { mensaje: Mensaje, ruta: PathBuf, tablas: Vec<Mensaje> },
    /// Una tabla del chat.
    Tabla { mensaje: Mensaje },
}

/// **Una hoja del chat abierta en el pin**: de donde viene y, desde que se
/// pulso «Editar», su editor (que se queda entre un modo y otro).
pub(crate) struct Editable {
    ubicacion: Ubicacion,
    proyecto: String,
    origen: Origen,
    editor: Option<Editor>,
    regional: Option<Regional>,
    /// Lo que se ve en modo ver es el `.xlsx` original (las hojas se casan
    /// con las tablas por el nombre).
    vista_original: bool,
    /// Se guardo algo que el modo ver aun no ensena.
    vista_atrasada: bool,
    /// Ya se dijo que el archivo original no cambia.
    avisado: bool,
    /// Abrir ya en modo editar (una tabla recien creada).
    pub(crate) al_abrir_editar: bool,
}

/// **Toca una burbuja del chat**: si es un libro o una tabla, la abre como
/// pin (en su hilo). `false` si no es una hoja o no esta en este equipo.
pub(crate) fn abrir_desde_chat(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    proyecto: &str,
    mensajes: &[Mensaje],
    indice: usize,
) -> bool {
    abrir(idioma, ubicacion, proyecto, mensajes, indice, false)
}

/// Lo mismo, pero ya editando: una tabla recien creada, que nace vacia y es
/// para escribir en ella (el movil abre la suya asi).
pub(crate) fn abrir_editando(idioma: pixpin_store::Idioma, ubicacion: &Ubicacion, proyecto: &str, mensajes: &[Mensaje], indice: usize) -> bool {
    abrir(idioma, ubicacion, proyecto, mensajes, indice, true)
}

fn abrir(idioma: pixpin_store::Idioma, ubicacion: &Ubicacion, proyecto: &str, mensajes: &[Mensaje], indice: usize, editando: bool) -> bool {
    let Some(m) = mensajes.get(indice) else { return false };
    let origen = match que_abrir(m) {
        Some(Que::Tabla) => Origen::Tabla { mensaje: m.clone() },
        Some(Que::Libro) => {
            let Some(ruta) = m
                .ruta
                .as_deref()
                .and_then(|r| pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), proyecto, r))
                .filter(|r| r.is_file())
            else {
                return false;
            };
            let tablas = libro::tablas_hechas(mensajes, m).into_iter().map(|i| mensajes[i].clone()).collect();
            Origen::Libro { mensaje: m.clone(), ruta, tablas }
        }
        None => return false,
    };
    tracing::info!(libro = matches!(origen, Origen::Libro { .. }), editando, "hoja del chat: se abre como pin");
    let mut ed = Editable::nuevo(ubicacion.clone(), proyecto.to_string(), origen);
    ed.al_abrir_editar = editando;
    crate::visor_hoja::lanzar_editable(idioma, ed);
    true
}

/// Como acaba el modo editar.
pub(crate) enum Vuelta {
    Cerrar,
    /// Volver a ver, con esta hoja; y el libro nuevo si lo editado cambio
    /// lo que se ve.
    Ver { activa: usize, libro: Option<Libro> },
}

impl Editable {
    pub(crate) fn nuevo(ubicacion: Ubicacion, proyecto: String, origen: Origen) -> Editable {
        Editable {
            ubicacion,
            proyecto,
            origen,
            editor: None,
            regional: None,
            vista_original: false,
            vista_atrasada: false,
            avisado: false,
            al_abrir_editar: false,
        }
    }

    /// El archivo del libro, para «Abrir con su programa».
    pub(crate) fn ruta_original(&self) -> Option<PathBuf> {
        match &self.origen {
            Origen::Libro { ruta, .. } => Some(ruta.clone()),
            Origen::Tabla { .. } => None,
        }
    }

    /// El nombre para la barra del pin.
    pub(crate) fn nombre(&self) -> String {
        let n = match &self.origen {
            Origen::Libro { mensaje, ruta, .. } => {
                if mensaje.nombre.trim().is_empty() {
                    pixpin_docs::nombre(ruta)
                } else {
                    mensaje.nombre.clone()
                }
            }
            Origen::Tabla { mensaje } => mensaje.nombre.clone(),
        };
        let n = crate::lector_pin::nombre_limpio(&n);
        if n.trim().is_empty() { "Tabla".into() } else { n }
    }

    /// **Lo que se ve en modo ver.** Un `.xlsx` que nadie edito, el
    /// original: se abre igual de rapido que desde el Explorador. Lo demas,
    /// sus tablas en un `.xlsx` hecho en memoria.
    pub(crate) fn libro_para_ver(&mut self, regional: Regional) -> Result<Libro, String> {
        self.regional = Some(regional.clone());
        if let Origen::Libro { ruta, tablas, .. } = &self.origen
            && crate::visor_hoja::se_abre_fichero(ruta)
            && !tablas.iter().any(libro::editada)
        {
            match Libro::abrir(ruta, regional) {
                Ok(l) => {
                    self.vista_original = true;
                    return Ok(l);
                }
                Err(e) => tracing::warn!(%e, "el visor no lee el libro; se prueba con sus tablas"),
            }
        }
        self.vista_original = false;
        self.montar_editor()?;
        self.libro_de_tablas()
    }

    /// Las tablas del editor pasadas a un libro para el visor.
    fn libro_de_tablas(&self) -> Result<Libro, String> {
        let ed = self.editor.as_ref().ok_or("sin tablas")?;
        let hojas: Vec<pixpin_proyecto::tabla_xlsx::HojaParaVer> = ed
            .hojas
            .iter()
            .map(|h| pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre: libro::nombre_de_pestana(&h.nombre), tabla: &h.tabla, valores: h.valores_hechos() })
            .collect();
        let t = Instant::now();
        let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&hojas, ed.activa).map_err(|e| e.to_string())?;
        let regional = self.regional.clone().unwrap_or_else(Regional::es_pe);
        let l = Libro::desde_bytes(bytes, regional).map_err(|e| e.to_string())?;
        tracing::info!(ms = t.elapsed().as_millis() as u64, hojas = hojas.len(), "hoja del chat: tablas pasadas al visor");
        Ok(l)
    }

    /// **Monta el editor** si no lo estaba: las tablas que ya hay en el chat
    /// o, la primera vez, el libro leido como tablas. No se relee nunca.
    fn montar_editor(&mut self) -> Result<(), String> {
        if self.editor.is_some() {
            return Ok(());
        }
        let t = Instant::now();
        let hojas = match &self.origen {
            Origen::Tabla { mensaje } => {
                let tabla = Tabla::leer(&mensaje.texto).map_err(|e| format!("tabla que no se entiende: {e}"))?;
                let nombre = if tabla.nombre.trim().is_empty() { mensaje.nombre.clone() } else { tabla.nombre.clone() };
                vec![HojaEditable::nueva(nombre, tabla, Some(mensaje.id.clone()))]
            }
            Origen::Libro { tablas, .. } if !tablas.is_empty() => tablas
                .iter()
                .map(|m| {
                    let tabla = Tabla::leer(&m.texto).map_err(|e| format!("tabla que no se entiende: {e}"))?;
                    Ok(HojaEditable::nueva(m.nombre.clone(), tabla, Some(m.id.clone())))
                })
                .collect::<Result<Vec<_>, String>>()?,
            Origen::Libro { mensaje, ruta, .. } => {
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                pixpin_proyecto::importar_hojas::leer(ruta, &mensaje.nombre, ahora)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|h| HojaEditable::nueva(h.nombre, h.tabla, None))
                    .collect()
            }
        };
        tracing::info!(ms = t.elapsed().as_millis() as u64, hojas = hojas.len(), "hoja del chat: tablas listas para editar");
        self.editor = Some(Editor::nuevo(hojas, 0).ok_or("el libro no tiene hojas")?);
        Ok(())
    }

    /// Si editar este libro deja sus tablas nuevas en el chat (la primera
    /// vez): es cuando hay que decir que el original no cambia.
    fn primera_edicion_de_libro(&self) -> bool {
        matches!(self.origen, Origen::Libro { .. }) && self.editor.as_ref().is_some_and(|e| e.hojas.iter().any(|h| h.id_mensaje.is_none()))
    }

    /// **Guarda lo cambiado en el chat**: cada tabla tocada en su mensaje
    /// (`cuaderno::cambiar`, con el cerrojo del cuaderno) y, si el libro
    /// aun no tenia tablas, todas sus hojas como mensajes nuevos con la
    /// referencia del libro. Despues avisa al chat para que lo relea.
    fn guardar(&mut self) -> Result<(), String> {
        let Some(ed) = self.editor.as_mut() else { return Ok(()) };
        ed.confirmar();
        if !ed.tocado() {
            return Ok(());
        }
        let raiz = self.ubicacion.raiz();
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &self.proyecto);
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let nuevas = ed.hojas.iter().any(|h| h.id_mensaje.is_none());
        if nuevas && let Origen::Libro { mensaje, tablas, .. } = &mut self.origen {
            let numero = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
                .map(|c| c.siguiente_numero(Some(&self.proyecto)))
                .unwrap_or(1);
            let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC").map(|i| i.yo.codigo()).unwrap_or_default();
            let sello = pixpin_proyecto::cuaderno::Sello { cuando: ahora, numero, aparato, proyecto: self.proyecto.clone() };
            // La hoja cambiada lleva un `tocado` posterior a la hora de su
            // mensaje (`libro::editada`); las demas, la de leerla.
            for (i, h) in ed.hojas.iter_mut().enumerate().filter(|(_, h)| h.tocada) {
                h.tabla.tocado = editada_a(ahora, sello.cuando + i as i64);
            }
            let listas = ed
                .hojas
                .iter()
                .map(|h| Ok(libro::HojaLista { nombre: h.nombre.clone(), documento: h.tabla.escribir().map_err(|e| e.to_string())? }))
                .collect::<Result<Vec<_>, String>>()?;
            let nuevos = libro::mensajes_de_hojas(mensaje, &listas, &sello);
            let lineas = nuevos.iter().map(pixpin_proyecto::cuaderno::linea_de).collect::<std::io::Result<Vec<_>>>().map_err(|e| e.to_string())?;
            pixpin_proyecto::cuaderno::anadir_lineas(&carpeta, &lineas).map_err(|e| e.to_string())?;
            for (h, m) in ed.hojas.iter_mut().zip(&nuevos) {
                h.id_mensaje = Some(m.id.clone());
                h.tocada = false;
            }
            *tablas = nuevos;
            tracing::info!(hojas = ed.hojas.len(), "hoja del chat: el libro editado queda como tablas del chat");
        } else {
            for h in ed.hojas.iter_mut().filter(|h| h.tocada) {
                let Some(id) = h.id_mensaje.clone() else { continue };
                let tabla = &mut h.tabla;
                match pixpin_proyecto::cuaderno::cambiar(&carpeta, &id, |m| {
                    tabla.tocado = editada_a(ahora, m.cuando);
                    match tabla.escribir() {
                        Ok(t) => {
                            m.texto = t;
                            true
                        }
                        Err(e) => {
                            tracing::warn!(?e, "hoja del chat: tabla que no se pudo escribir");
                            false
                        }
                    }
                }) {
                    Ok(Some(_)) => h.tocada = false,
                    // Se borro del chat mientras se editaba: no se resucita.
                    Ok(None) => tracing::warn!("hoja del chat: la tabla ya no esta en el chat; no se guarda"),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        self.vista_atrasada = true;
        crate::ventana_chat::refrescar();
        Ok(())
    }
}

/// El `tocado` de una tabla guardada: ahora, pero siempre claramente despues
/// de la hora de su mensaje (`cuando`), para que se sepa que se edito
/// (`libro::editada`) aunque se guarde en el mismo segundo en que nacio.
fn editada_a(ahora: i64, cuando: i64) -> i64 {
    ahora.max(cuando + 2_001)
}

// ---------------------------------------------------------------------------
// El modo editar: la ventana

/// Botones propios de la barra en modo editar.
const BOTON_VER: u8 = 10;
const BOTON_DESHACER: u8 = 11;
const BOTON_REHACER: u8 = 12;
const BOTON_NEGRITA: u8 = 13;
const BOTON_COPIAR: u8 = 14;

/// Medidas en pixeles logicos.
const ALTO_FORMULA: f32 = 26.0;
const ALTO_PESTANAS: f32 = 28.0;
const ANCHO_NUMEROS: f32 = 44.0;
const ALTO_LETRAS: f32 = 20.0;
const TAM_BARRA: f32 = 12.5;
/// Calibri 11, la letra de Excel (y la del visor) en pixeles al 100 %.
const TAM_CELDA: f32 = 11.0 * 96.0 / 72.0;
const LETRA: &str = "Calibri";
/// Dos clics en la misma celda antes de esto: editarla.
const DOBLE_CLIC: Duration = Duration::from_millis(450);
const AVISO: Duration = Duration::from_millis(1800);
const AVISO_LARGO: Duration = Duration::from_millis(5000);

const FONDO_BARRA: Color = Color { r: 0.953, g: 0.953, b: 0.953, a: 1.0 };
const LINEA: Color = Color { r: 0.831, g: 0.831, b: 0.831, a: 1.0 };
const CUADRICULA: Color = Color { r: 0.886, g: 0.886, b: 0.886, a: 1.0 };
const ENCABEZADO: Color = Color { r: 0.961, g: 0.961, b: 0.961, a: 1.0 };
const ENCABEZADO_ELEGIDO: Color = Color { r: 0.827, g: 0.886, b: 0.851, a: 1.0 };
const TEXTO: Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 1.0 };
const TENUE: Color = Color { r: 0.42, g: 0.42, b: 0.42, a: 1.0 };
const VERDE: Color = Color { r: 0.129, g: 0.451, b: 0.275, a: 1.0 };
const RANGO: Color = Color { r: 0.129, g: 0.451, b: 0.275, a: 0.12 };
const ERROR: Color = Color { r: 0.75, g: 0.1, b: 0.1, a: 1.0 };

const VK_RETROCESO: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_REPAG: u32 = 0x21;
const VK_AVPAG: u32 = 0x22;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_SUPR: u32 = 0x2E;
const VK_F2: u32 = 0x71;
const VK_F11: u32 = 0x7A;

/// Lo que se hace con una tecla en modo editar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionEd {
    /// Mover la celda activa (dx, dy, alargando el rango).
    Mover(i32, i32, bool),
    /// Avanzar o retroceder una pantalla.
    Pagina(bool),
    /// Ctrl+Inicio.
    Inicio,
    Empezar,
    Cancelar,
    /// Esc sin escribir: volver a ver.
    Ver,
    Retroceso,
    Borrar,
    Deshacer,
    Rehacer,
    Copiar,
    Cortar,
    Pegar,
    Negrita,
    Alinear(Alineacion),
    Todo,
    PantallaCompleta,
}

/// **Que hace una tecla en modo editar**, sabiendo si se esta escribiendo en
/// una celda. Las letras sueltas no estan: llegan como caracteres.
pub fn accion_de_tecla(vk: u32, shift: bool, ctrl: bool, escribiendo: bool) -> Option<AccionEd> {
    use AccionEd::*;
    Some(match (vk, ctrl) {
        (0x5A, true) if shift => Rehacer,
        (0x5A, true) => Deshacer,
        (0x59, true) => Rehacer,
        (0x43, true) => Copiar,
        (0x58, true) => Cortar,
        (0x56, true) => Pegar,
        // Ctrl+N en la Excel espanola, Ctrl+B en la inglesa.
        (0x42 | 0x4E, true) => Negrita,
        (0x41, true) if !escribiendo => Todo,
        // Ctrl+Mayus+L/E/R, como Google Sheets.
        (0x4C, true) if shift => Alinear(Alineacion::Izquierda),
        (0x45, true) if shift => Alinear(Alineacion::Centro),
        (0x52, true) if shift => Alinear(Alineacion::Derecha),
        (VK_INICIO, true) => Inicio,
        (VK_ESCAPE, _) if escribiendo => Cancelar,
        (VK_ESCAPE, _) => Ver,
        (VK_ENTRAR, _) => Mover(0, if shift { -1 } else { 1 }, false),
        (VK_TAB, _) => Mover(if shift { -1 } else { 1 }, 0, false),
        (VK_IZQUIERDA, false) => Mover(-1, 0, shift),
        (VK_DERECHA, false) => Mover(1, 0, shift),
        (VK_ARRIBA, false) => Mover(0, -1, shift),
        (VK_ABAJO, false) => Mover(0, 1, shift),
        (VK_REPAG, _) => Pagina(false),
        (VK_AVPAG, _) => Pagina(true),
        (VK_F2, _) => Empezar,
        (VK_RETROCESO, _) => Retroceso,
        (VK_SUPR, _) if !escribiendo => Borrar,
        (VK_F11, _) => PantallaCompleta,
        _ => return None,
    })
}

/// La hoja que se edita al pulsar «Editar» viendo la hoja `vista`: la de su
/// nombre si se ve el original, la misma si se ven las tablas.
pub fn hoja_al_editar(vista_original: bool, nombres_vista: &[&str], nombres_tablas: &[&str], vista: usize) -> usize {
    if vista_original {
        nombres_vista.get(vista).map_or(0, |n| libro::tabla_de_hoja(nombres_tablas, n))
    } else {
        vista.min(nombres_tablas.len().saturating_sub(1))
    }
}

/// Y al reves, al volver a ver: la hoja del libro que es la tabla `tabla`.
pub fn hoja_al_ver(vista_original: bool, nombres_vista: &[&str], nombres_tablas: &[&str], tabla: usize, antes: usize) -> usize {
    if !vista_original {
        return tabla.min(nombres_vista.len().saturating_sub(1));
    }
    if nombres_tablas.len() <= 1 {
        return antes;
    }
    let Some(n) = nombres_tablas.get(tabla) else { return antes };
    nombres_vista.iter().position(|v| *v == libro::nombre_de_pestana(n)).unwrap_or(antes)
}

/// Donde va cada cosa en la ventana, en pixeles.
#[derive(Debug, Clone, Copy)]
struct Zonas {
    formula: RectF,
    /// Las celdas, sin los encabezados.
    celdas: RectF,
    letras: RectF,
    numeros: RectF,
    pestanas: RectF,
}

fn zonas(ancho: f32, alto: f32, e: f32) -> Zonas {
    let (af, ap) = ((ALTO_FORMULA * e).round(), (ALTO_PESTANAS * e).round());
    let (an, al) = ((ANCHO_NUMEROS * e).round(), (ALTO_LETRAS * e).round());
    let y0 = af;
    let alto_hoja = (alto - af - ap).max(0.0);
    Zonas {
        formula: RectF { x: 0.0, y: 0.0, ancho, alto: af },
        letras: RectF { x: an, y: y0, ancho: (ancho - an).max(0.0), alto: al },
        numeros: RectF { x: 0.0, y: y0 + al, ancho: an, alto: (alto_hoja - al).max(0.0) },
        celdas: RectF { x: an, y: y0 + al, ancho: (ancho - an).max(0.0), alto: (alto_hoja - al).max(0.0) },
        pestanas: RectF { x: 0.0, y: (alto - ap).max(af), ancho, alto: ap },
    }
}

fn dentro(r: &RectF, (x, y): (f32, f32)) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.ancho && y < r.y + r.alto
}

/// El estado de la ventana en modo editar (lo que no es la hoja).
struct Ventana {
    e: f32,
    ancho: f32,
    alto: f32,
    raton: (f32, f32),
    /// Arrastrando para elegir un rango.
    eligiendo: bool,
    ultimo_clic: Option<(Ref, Instant)>,
    aviso: Option<(String, Instant)>,
    pestanas: Vec<(RectF, usize)>,
    decimal: char,
}

impl Ventana {
    fn zonas(&self) -> Zonas {
        zonas(self.ancho, self.alto, self.e)
    }

    /// Lo que se ve de las celdas, en pixeles de hoja al 100 %.
    fn visible(&self) -> (f32, f32) {
        let z = self.zonas();
        (z.celdas.ancho / self.e, z.celdas.alto / self.e)
    }

    /// La celda bajo el raton (acotada a las celdas al arrastrar).
    fn celda_en(&self, h: &HojaEditable, (x, y): (f32, f32), acotada: bool) -> Option<Ref> {
        let z = self.zonas();
        let (mut px, mut py) = (x - z.celdas.x, y - z.celdas.y);
        if acotada {
            px = px.clamp(0.0, (z.celdas.ancho - 1.0).max(0.0));
            py = py.clamp(0.0, (z.celdas.alto - 1.0).max(0.0));
        } else if px < 0.0 || py < 0.0 || px >= z.celdas.ancho || py >= z.celdas.alto {
            return None;
        }
        Some(Ref { columna: h.columna_en(h.x + px / self.e), fila: HojaEditable::fila_en(h.y + py / self.e) })
    }

    fn avisar(&mut self, texto: String, cuanto: Duration) {
        self.aviso = Some((texto, Instant::now() + cuanto));
    }
}

/// **El modo editar** en la ventana del pin, hasta «Ver» o cerrar. Guarda
/// al salir (si se cambio algo).
pub(crate) fn editar(marco: &mut Marco, motor: &MotorRender, textos: &Catalogo, ed: &mut Editable, visto: &Libro, vista: usize) -> Vuelta {
    let entrada = Instant::now();
    let mut v = Ventana {
        e: marco.e,
        ancho: marco.area.ancho as f32,
        alto: marco.area.alto as f32,
        raton: (0.0, 0.0),
        eligiendo: false,
        ultimo_clic: None,
        aviso: None,
        pestanas: Vec::new(),
        decimal: pixpin_shell::entorno::separador_decimal(),
    };
    if ed.editor.is_none() {
        // Leer un libro grande tarda: que se vea que se esta en ello.
        pintar_espera(marco, motor, &textos.t("hoja-preparando"));
        if let Err(e) = ed.montar_editor() {
            tracing::warn!(%e, "hoja del chat: no se puede editar");
            return Vuelta::Ver { activa: vista, libro: None };
        }
    }
    let nombres_vista: Vec<&str> = visto.hojas.iter().map(|h| h.nombre.as_str()).collect();
    if let Some(e) = ed.editor.as_mut() {
        let nombres: Vec<String> = e.hojas.iter().map(|h| h.nombre.clone()).collect();
        let nombres: Vec<&str> = nombres.iter().map(String::as_str).collect();
        let i = hoja_al_editar(ed.vista_original, &nombres_vista, &nombres, vista);
        e.cambiar_hoja(i);
    }
    if ed.primera_edicion_de_libro() && !ed.avisado {
        ed.avisado = true;
        v.avisar(textos.t("hoja-editar-aviso"), AVISO_LARGO);
    }
    tracing::info!(ms = entrada.elapsed().as_millis() as u64, "hoja del chat: modo editar");

    let mut hay_que_pintar = true;
    let mut salida: Option<bool> = None; // Some(true): cerrar; Some(false): ver
    while salida.is_none() {
        pixpin_shell::overlay::bombear_pendientes();
        let Some(editor) = ed.editor.as_mut() else { break };
        #[cfg(test)]
        match prueba::latido(marco.ventana.handle().0 as isize, marco.area, true) {
            Some(prueba::VER) => salida = Some(false),
            Some(prueba::CERRAR) => salida = Some(true),
            Some(prueba::ESCRIBIR) => {
                pruebas::escribir_muestra(editor);
                editor.hoja_mut().seguir(v.visible());
                hay_que_pintar = true;
            }
            _ => {}
        }
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if marco.pin.es_de_la_barra(hwnd) {
                let (accion, _) = marco.pin.evento_barra(&evento, &mut marco.ventana);
                match accion {
                    Some(AccionPin::Propio(BOTON_VER)) => salida = Some(false),
                    Some(AccionPin::Propio(BOTON_DESHACER)) => {
                        editor.deshacer();
                    }
                    Some(AccionPin::Propio(BOTON_REHACER)) => {
                        editor.rehacer();
                    }
                    Some(AccionPin::Propio(BOTON_NEGRITA)) => {
                        editor.alternar_negrita();
                    }
                    Some(AccionPin::Propio(BOTON_COPIAR)) => copiar(editor, &mut v, textos),
                    Some(AccionPin::PantallaCompleta) => marco.pin.alternar_pantalla_completa(&mut marco.ventana),
                    Some(AccionPin::Fijar) => marco.pin.alternar_fijado(&marco.ventana),
                    Some(AccionPin::Cerrar) => salida = Some(true),
                    _ => {}
                }
                hay_que_pintar = true;
                continue;
            }
            if hwnd != marco.ventana.handle() {
                continue;
            }
            if marco.pin.evento_ventana(&evento, &mut marco.ventana).consumido {
                hay_que_pintar = true;
                continue;
            }
            let area = marco.area;
            let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
            match evento {
                EventoOverlay::Cerrar => salida = Some(true),
                EventoOverlay::RatonMovido(p) => {
                    v.raton = local(p);
                    if v.eligiendo
                        && let Some(r) = v.celda_en(editor.hoja(), v.raton, true)
                        && editor.hoja().sel.activa != r
                    {
                        editor.hoja_mut().sel.activa = r;
                        let vis = v.visible();
                        editor.hoja_mut().seguir(vis);
                        hay_que_pintar = true;
                    }
                    marco.ventana.poner_cursor(FormaCursorWin::Flecha);
                }
                EventoOverlay::BotonPulsado(p) => {
                    v.raton = local(p);
                    if pulsar(editor, &mut v) {
                        marco.ventana.capturar_raton();
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::BotonSoltado(p) => {
                    v.raton = local(p);
                    if v.eligiendo {
                        v.eligiendo = false;
                        marco.ventana.soltar_raton();
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Rueda(delta) | EventoOverlay::RuedaHorizontal(delta) => {
                    let lado = matches!(evento, EventoOverlay::RuedaHorizontal(_)) || pixpin_shell::entrada::modificadores_pulsados().shift;
                    rueda(editor.hoja_mut(), delta, lado, matches!(evento, EventoOverlay::RuedaHorizontal(_)));
                    hay_que_pintar = true;
                }
                // El caracter de la `E` que trajo hasta aqui no se escribe.
                EventoOverlay::Caracter(_) if entrada.elapsed() < Duration::from_millis(250) => {}
                EventoOverlay::Caracter(c) if c >= ' ' => {
                    editor.teclear(c);
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, shift, ctrl, .. } => {
                    if let Some(a) = accion_de_tecla(vk, shift, ctrl, editor.edicion.is_some()) {
                        match a {
                            AccionEd::Ver => salida = Some(false),
                            AccionEd::PantallaCompleta => marco.pin.alternar_pantalla_completa(&mut marco.ventana),
                            otra => hacer(editor, otra, &mut v, textos),
                        }
                    }
                    hay_que_pintar = true;
                }
                _ => {}
            }
        }
        if salida.is_some() {
            break;
        }
        // Movida o estirada desde fuera: el pin la sigue.
        marco.pin.seguir_ventana(&mut marco.ventana);
        if marco.pin.area != marco.area {
            if (marco.pin.area.ancho, marco.pin.area.alto) != (marco.area.ancho, marco.area.alto) {
                let _ = marco.superficie.redimensionar(marco.pin.area.ancho, marco.pin.area.alto);
            }
            marco.area = marco.pin.area;
            v.ancho = marco.area.ancho as f32;
            v.alto = marco.area.alto as f32;
            hay_que_pintar = true;
        }
        let botones = [
            BotonLector { id: BOTON_VER, icono: &material::VISIBILITY, encendido: false },
            BotonLector { id: BOTON_DESHACER, icono: &material::UNDO, encendido: false },
            BotonLector { id: BOTON_REHACER, icono: &material::REDO, encendido: false },
            BotonLector { id: BOTON_NEGRITA, icono: &material::FORMAT_BOLD, encendido: negrita_activa(editor) },
            BotonLector { id: BOTON_COPIAR, icono: &material::CONTENT_COPY, encendido: false },
        ];
        marco.pin.actualizar(motor, &marco.nombre, &botones);
        if v.aviso.as_ref().is_some_and(|(_, hasta)| Instant::now() >= *hasta) {
            v.aviso = None;
            hay_que_pintar = true;
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = marco.superficie.empezar(motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| pintar(p, editor, &mut v, textos));
                let _ = marco.superficie.presentar();
            }
        }
        let tope = if v.eligiendo {
            16
        } else if marco.pin.quiere_reloj() || v.aviso.is_some() {
            50
        } else {
            200
        };
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }
    if v.eligiendo {
        marco.ventana.soltar_raton();
    }

    // Al salir se guarda: en el chat, nunca en el archivo.
    if let Err(e) = ed.guardar() {
        tracing::error!(%e, "hoja del chat: no se pudo guardar; los cambios siguen en el pin");
    }
    if salida == Some(true) {
        return Vuelta::Cerrar;
    }
    let tabla = ed.editor.as_ref().map_or(0, |e| e.activa);
    let nombres: Vec<String> = ed.editor.as_ref().map(|e| e.hojas.iter().map(|h| h.nombre.clone()).collect()).unwrap_or_default();
    let nombres: Vec<&str> = nombres.iter().map(String::as_str).collect();
    if ed.vista_atrasada {
        ed.vista_atrasada = false;
        match ed.libro_de_tablas() {
            Ok(l) => {
                ed.vista_original = false;
                return Vuelta::Ver { activa: tabla, libro: Some(l) };
            }
            Err(e) => tracing::warn!(%e, "hoja del chat: no se pudo rehacer lo que se ve"),
        }
    }
    Vuelta::Ver { activa: hoja_al_ver(ed.vista_original, &nombres_vista, &nombres, tabla, vista), libro: None }
}

/// Si la celda activa esta en negrita (el boton se enciende).
fn negrita_activa(e: &Editor) -> bool {
    let h = e.hoja();
    h.tabla.estilos.get(&pixpin_proyecto::tabla::ref_a(h.sel.activa)).is_some_and(|s| s.n)
}

/// Un fotograma con solo un aviso, mientras se leen las tablas.
fn pintar_espera(marco: &mut Marco, motor: &MotorRender, texto: &str) {
    let (w, h, e) = (marco.area.ancho as f32, marco.area.alto as f32, marco.e);
    if let Ok(destino) = marco.superficie.empezar(motor) {
        let _ = motor.dibujar(&destino, |p: &Pintor| {
            p.limpiar(Color::BLANCO);
            crate::lector::aviso_en(p, texto, w / 2.0, h / 2.0 - 16.0 * e, e);
        });
        let _ = marco.superficie.presentar();
    }
}

/// Un clic: una pestana, un encabezado o una celda (dos en la misma,
/// editarla). `true` si empieza a elegir arrastrando.
fn pulsar(editor: &mut Editor, v: &mut Ventana) -> bool {
    let z = v.zonas();
    let p = v.raton;
    if dentro(&z.pestanas, p) {
        if let Some((_, i)) = v.pestanas.iter().find(|(c, _)| dentro(c, p)) {
            editor.cambiar_hoja(*i);
        }
        return false;
    }
    let extender = pixpin_shell::entrada::modificadores_pulsados().shift;
    // Un encabezado elige su columna o su fila hasta lo escrito.
    if dentro(&z.letras, p) || dentro(&z.numeros, p) {
        let (cols, filas) = editor.hoja().tabla.tamano();
        let Some(r) = v.celda_en(editor.hoja(), p, true) else { return false };
        let h = editor.hoja();
        let (desde, hasta) = if dentro(&z.letras, p) {
            (Ref { columna: r.columna, fila: 0 }, Ref { columna: r.columna, fila: filas.saturating_sub(1).max(h.sel.activa.fila) })
        } else {
            (Ref { columna: 0, fila: r.fila }, Ref { columna: cols.saturating_sub(1).max(h.sel.activa.columna), fila: r.fila })
        };
        editor.elegir(desde, false);
        editor.elegir(hasta, true);
        return false;
    }
    let Some(r) = v.celda_en(editor.hoja(), p, false) else { return false };
    let doble = v.ultimo_clic.is_some_and(|(c, t)| c == r && t.elapsed() < DOBLE_CLIC);
    if editor.edicion.is_some() && editor.hoja().sel.activa == r {
        // Un clic en la celda que se esta escribiendo no la deja.
        return false;
    }
    editor.elegir(r, extender);
    if doble {
        v.ultimo_clic = None;
        editor.empezar(true);
        return false;
    }
    v.ultimo_clic = Some((r, Instant::now()));
    v.eligiendo = true;
    true
}

/// La rueda: tres filas por muesca; de lado con Mayus o con el gesto
/// horizontal del panel.
fn rueda(h: &mut HojaEditable, delta: i32, de_lado: bool, horizontal: bool) {
    let m = crate::lector::muescas(delta);
    if de_lado {
        let signo = if horizontal { 1.0 } else { -1.0 };
        h.x = (h.x + signo * m * 120.0).max(0.0);
    } else {
        h.y = (h.y - m * 3.0 * pixpin_proyecto::tabla_xlsx::ALTO_FILA as f32).max(0.0);
    }
}

/// Lo que piden las teclas que no son mover la ventana.
fn hacer(editor: &mut Editor, a: AccionEd, v: &mut Ventana, textos: &Catalogo) {
    let vis = v.visible();
    match a {
        AccionEd::Mover(dx, dy, ext) => editor.mover(dx, dy, ext),
        AccionEd::Pagina(abajo) => {
            let filas = (vis.1 / pixpin_proyecto::tabla_xlsx::ALTO_FILA as f32).floor().max(1.0) as i32;
            editor.mover(0, if abajo { filas } else { -filas }, false);
        }
        AccionEd::Inicio => editor.elegir(Ref { columna: 0, fila: 0 }, false),
        AccionEd::Empezar => editor.empezar(true),
        AccionEd::Cancelar => editor.cancelar(),
        AccionEd::Retroceso => editor.retroceso(),
        AccionEd::Borrar => {
            if !editor.borrar_seleccion() && editor.hoja().sel.celdas() > editor::MAXIMO_RANGO {
                v.avisar(textos.t("hoja-rango-grande"), AVISO);
            }
        }
        AccionEd::Deshacer => {
            editor.deshacer();
        }
        AccionEd::Rehacer => {
            editor.rehacer();
        }
        AccionEd::Copiar => copiar(editor, v, textos),
        AccionEd::Cortar => {
            copiar(editor, v, textos);
            editor.borrar_seleccion();
        }
        AccionEd::Pegar => pegar(editor),
        AccionEd::Negrita => {
            editor.alternar_negrita();
        }
        AccionEd::Alinear(al) => {
            editor.alinear(al);
        }
        AccionEd::Todo => editor.elegir_todo(),
        AccionEd::Ver | AccionEd::PantallaCompleta => {}
    }
    editor.hoja_mut().seguir(vis);
}

/// Copia lo elegido: los valores con tabuladores y las formulas en la tabla
/// HTML (para pegar en Excel o Sheets). Escribiendo, lo escrito.
fn copiar(editor: &Editor, v: &mut Ventana, textos: &Catalogo) {
    let hecho = match &editor.edicion {
        Some(t) => pixpin_codec::portapapeles::copiar_texto(t),
        None => {
            let h = editor.hoja();
            if h.sel.celdas() > editor::MAXIMO_RANGO {
                v.avisar(textos.t("hoja-rango-grande"), AVISO);
                return;
            }
            let (tsv, html) = pixpin_proyecto::portapapeles_tabla::copiar(&h.tabla, h.sel.ancla, h.sel.activa, v.decimal);
            pixpin_codec::portapapeles::tabla::copiar_tabla(&tsv, &html)
        }
    };
    match hecho {
        Ok(()) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("celdas", editor.hoja().sel.celdas());
            v.avisar(textos.t_args("hoja-copiado", &args), AVISO);
        }
        Err(e) => tracing::warn!(?e, "no se pudo copiar de la hoja"),
    }
}

/// Pega desde Excel, Sheets o LibreOffice (o texto). Escribiendo, el texto
/// entra en lo que se escribe, en una linea.
fn pegar(editor: &mut Editor) {
    let Some((html, texto)) = pixpin_codec::portapapeles::tabla::leer_tabla() else { return };
    if let Some(edicion) = editor.edicion.as_mut() {
        if let Some(t) = texto {
            edicion.push_str(t.replace(['\r', '\n', '\t'], " ").trim_end());
        }
        return;
    }
    let Some(filas) = pixpin_proyecto::portapapeles_tabla::leer(html.as_deref(), texto.as_deref()) else { return };
    let hecho = editor.pegar(&filas);
    tracing::info!(filas = filas.len(), hecho, "pegado en la hoja del pin");
}

// ---------------------------------------------------------------------------
// Pintar

fn letra(negrita: bool) -> Letra<'static> {
    Letra { familia: LETRA, negrita, cursiva: false, interlineado: None }
}

/// `#rrggbb` a color.
fn color_de(f: &str) -> Option<Color> {
    let h = f.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(Color { r: ((n >> 16) & 0xFF) as f32 / 255.0, g: ((n >> 8) & 0xFF) as f32 / 255.0, b: (n & 0xFF) as f32 / 255.0, a: 1.0 })
}

/// **Las claves de `mapa` que caen en las esquinas** (columna0, fila0,
/// columna1, fila1; las ultimas sin incluir), con su celda. Recorre lo que
/// sea menos: el mapa si tiene menos celdas que la pantalla, o la pantalla
/// preguntando celda a celda. Una tabla de cuarenta mil celdas no se
/// recorre entera en cada fotograma.
pub fn celdas_visibles<V>(mapa: &std::collections::BTreeMap<String, V>, (c0, f0, c1, f1): (u32, u32, u32, u32)) -> Vec<(Ref, String)> {
    let area = (c1.saturating_sub(c0)) as usize * (f1.saturating_sub(f0)) as usize;
    if mapa.len() <= area {
        mapa.keys()
            .filter_map(|k| {
                let r = pixpin_proyecto::tabla::ref_de(k)?;
                ((c0..c1).contains(&r.columna) && (f0..f1).contains(&r.fila)).then(|| (r, k.clone()))
            })
            .collect()
    } else {
        let mut v = Vec::new();
        for fila in f0..f1 {
            for columna in c0..c1 {
                let r = Ref { columna, fila };
                let k = pixpin_proyecto::tabla::ref_a(r);
                if mapa.contains_key(&k) {
                    v.push((r, k));
                }
            }
        }
        v
    }
}

fn centrado_v(p: &Pintor, texto: &str, x: f32, caja: RectF, tam: f32, ancho: f32, color: Color) {
    let (_, h) = p.medir_texto("Ag", tam);
    p.texto_linea(texto, x, caja.y + (caja.alto - h) / 2.0, tam, ancho.max(0.0), color);
}

fn pintar(p: &Pintor, editor: &Editor, v: &mut Ventana, textos: &Catalogo) {
    let z = v.zonas();
    let e = v.e;
    let h = editor.hoja();
    p.limpiar(Color::BLANCO);

    // Las columnas y filas que se ven.
    let col0 = h.columna_en(h.x);
    let fila0 = HojaEditable::fila_en(h.y);
    let x_de = |c: u32| z.celdas.x + (h.x_de(c) - h.x) * e;
    let y_de = |f: u32| z.celdas.y + (HojaEditable::y_de(f) - h.y) * e;
    let mut col1 = col0;
    while x_de(col1) < z.celdas.x + z.celdas.ancho && col1 < pixpin_proyecto::importar_hojas::MAX_COLS - 1 {
        col1 += 1;
    }
    let mut fila1 = fila0;
    while y_de(fila1) < z.celdas.y + z.celdas.alto && fila1 < pixpin_proyecto::importar_hojas::MAX_FILAS - 1 {
        fila1 += 1;
    }
    let linea = 1.0_f32.max(e.floor());

    p.empujar_recorte(z.celdas);
    // Lo que se ve: las celdas de la pantalla, sin recorrer la tabla entera.
    let visibles = celdas_visibles(&h.tabla.celdas, (col0, fila0, col1, fila1));
    let con_estilo = celdas_visibles(&h.tabla.estilos, (col0, fila0, col1, fila1));
    // Los fondos de las celdas con color.
    for (r, k) in &con_estilo {
        let Some(c) = h.tabla.estilos.get(k).and_then(|s| s.f.as_deref()).and_then(color_de) else { continue };
        p.rellenar(RectF { x: x_de(r.columna), y: y_de(r.fila), ancho: h.ancho_de(r.columna) * e, alto: (y_de(r.fila + 1) - y_de(r.fila)) }, c);
    }
    // El rango elegido, en verde claro.
    let (c0, f0, c1, f1) = h.sel.esquinas();
    let caja_sel = RectF { x: x_de(c0), y: y_de(f0), ancho: x_de(c1 + 1) - x_de(c0), alto: y_de(f1 + 1) - y_de(f0) };
    if h.sel.celdas() > 1 {
        p.rellenar(caja_sel, RANGO);
    }
    // La cuadricula: lineas enteras, no cuatro por celda.
    for c in col0..=col1 {
        p.rellenar(RectF { x: x_de(c + 1) - linea, y: z.celdas.y, ancho: linea, alto: z.celdas.alto }, CUADRICULA);
    }
    for f in fila0..=fila1 {
        p.rellenar(RectF { x: z.celdas.x, y: y_de(f + 1) - linea, ancho: z.celdas.ancho, alto: linea }, CUADRICULA);
    }
    // Los textos.
    let tam = TAM_CELDA * e;
    let relleno = 4.0 * e;
    let alto_fila = y_de(1) - y_de(0);
    for (r, k) in &visibles {
        let r = *r;
        if editor.edicion.is_some() && r == h.sel.activa {
            continue;
        }
        let texto = h.mostrado(r, v.decimal);
        if texto.is_empty() {
            continue;
        }
        let est = h.tabla.estilos.get(k);
        let negrita = est.is_some_and(|s| s.n);
        let l = letra(negrita);
        let (w, th) = p.medir_con_letra(&texto, tam, pixpin_render::letras::SIN_PARTIR, &l);
        let caja = RectF { x: x_de(r.columna), y: y_de(r.fila), ancho: h.ancho_de(r.columna) * e, alto: alto_fila };
        let a = est.and_then(|s| s.a.as_deref()).unwrap_or(if h.es_numero(r) { "d" } else { "i" });
        let es_error = texto.starts_with('#') && texto.ends_with(['!', '?']);
        let x = match a {
            "c" => caja.x + (caja.ancho - w) / 2.0,
            "d" => caja.x + caja.ancho - relleno - w,
            _ => caja.x + relleno,
        };
        let y = caja.y + (caja.alto - th) / 2.0;
        let recortar = w > caja.ancho - 2.0 * relleno;
        if recortar {
            // Un texto largo se sale por las celdas vacias de al lado, como
            // en Excel (un numero no: se corta).
            let vacia = |c: u32| !h.tabla.celdas.contains_key(&pixpin_proyecto::tabla::ref_a(Ref { columna: c, fila: r.fila }));
            let (mut c0, mut c1) = (r.columna, r.columna);
            if !h.es_numero(r) {
                let falta = |c0: u32, c1: u32| x_de(c1 + 1) - x_de(c0) < w + 2.0 * relleno;
                let derecha = a != "d";
                let izquierda = a != "i";
                while falta(c0, c1) {
                    let mas_d = derecha && c1 + 1 < col1 && vacia(c1 + 1);
                    let mas_i = izquierda && c0 > col0 && vacia(c0 - 1);
                    if !mas_d && !mas_i {
                        break;
                    }
                    if mas_d {
                        c1 += 1;
                    }
                    if mas_i {
                        c0 -= 1;
                    }
                }
            }
            let caja_d = RectF { x: x_de(c0) + 1.0, ancho: (x_de(c1 + 1) - x_de(c0) - 2.0).max(0.0), ..caja };
            p.empujar_recorte(caja_d);
        }
        p.texto_con_letra(&texto, x, y, tam, pixpin_render::letras::SIN_PARTIR, &l, if es_error { ERROR } else { TEXTO });
        if recortar {
            p.soltar_recorte();
        }
    }
    // La celda que se escribe: en blanco encima, con lo tecleado y el cursor.
    let activa = h.sel.activa;
    let caja_act = RectF { x: x_de(activa.columna), y: y_de(activa.fila), ancho: h.ancho_de(activa.columna) * e, alto: alto_fila };
    if let Some(t) = &editor.edicion {
        let l = letra(false);
        let (w, th) = p.medir_con_letra(t, tam, pixpin_render::letras::SIN_PARTIR, &l);
        let ancho = (w + 2.0 * relleno + 2.0 * e).max(caja_act.ancho);
        let caja = RectF { ancho, ..caja_act };
        p.rellenar(caja, Color::BLANCO);
        let y = caja.y + (caja.alto - th) / 2.0;
        p.texto_con_letra(t, caja.x + relleno, y, tam, pixpin_render::letras::SIN_PARTIR, &l, TEXTO);
        p.rellenar(RectF { x: caja.x + relleno + w + 1.0, y: y + 1.0, ancho: (1.0 * e).max(1.0), alto: th - 2.0 }, TEXTO);
    }
    // El marco de lo elegido y de la celda activa.
    let grueso = (2.0 * e).round().max(2.0);
    p.trazar(caja_sel, grueso, VERDE);
    if h.sel.celdas() > 1 {
        p.trazar(caja_act, 1.0, VERDE);
    }
    let lado = 6.0 * e;
    p.rellenar(RectF { x: caja_sel.x + caja_sel.ancho - lado / 2.0, y: caja_sel.y + caja_sel.alto - lado / 2.0, ancho: lado, alto: lado }, VERDE);
    p.soltar_recorte();

    // Los encabezados: letras arriba y numeros a la izquierda.
    p.rellenar(z.letras, ENCABEZADO);
    p.rellenar(z.numeros, ENCABEZADO);
    p.rellenar(RectF { x: 0.0, y: z.letras.y, ancho: z.numeros.ancho, alto: z.letras.alto }, ENCABEZADO);
    let tam_cab = 11.0 * e;
    p.empujar_recorte(z.letras);
    for c in col0..col1 {
        let x = x_de(c);
        let ancho = h.ancho_de(c) * e;
        if (c0..=c1).contains(&c) {
            p.rellenar(RectF { x, y: z.letras.y, ancho, alto: z.letras.alto }, ENCABEZADO_ELEGIDO);
        }
        let n = nombre_columna(c);
        let (w, th) = p.medir_texto(&n, tam_cab);
        p.texto(&n, x + (ancho - w) / 2.0, z.letras.y + (z.letras.alto - th) / 2.0, tam_cab, TENUE);
        p.rellenar(RectF { x: x + ancho - linea, y: z.letras.y, ancho: linea, alto: z.letras.alto }, LINEA);
    }
    p.soltar_recorte();
    p.empujar_recorte(z.numeros);
    for f in fila0..fila1 {
        let y = y_de(f);
        if (f0..=f1).contains(&f) {
            p.rellenar(RectF { x: 0.0, y, ancho: z.numeros.ancho, alto: alto_fila }, ENCABEZADO_ELEGIDO);
        }
        let n = (f + 1).to_string();
        let (w, th) = p.medir_texto(&n, tam_cab);
        p.texto(&n, (z.numeros.ancho - w) / 2.0, y + (alto_fila - th) / 2.0, tam_cab, TENUE);
        p.rellenar(RectF { x: 0.0, y: y + alto_fila - linea, ancho: z.numeros.ancho, alto: linea }, LINEA);
    }
    p.soltar_recorte();
    p.rellenar(RectF { x: z.celdas.x - linea, y: z.letras.y, ancho: linea, alto: z.letras.alto + z.numeros.alto }, LINEA);
    p.rellenar(RectF { x: 0.0, y: z.celdas.y - linea, ancho: v.ancho, alto: linea }, LINEA);

    // La barra de formulas: la celda y lo que tiene escrito (la formula).
    let f = z.formula;
    p.rellenar(f, FONDO_BARRA);
    p.rellenar(RectF { y: f.y + f.alto - 1.0, alto: 1.0, ..f }, LINEA);
    let tb = TAM_BARRA * e;
    let referencia = if h.sel.celdas() > 1 {
        format!("{}{}:{}{}", nombre_columna(c0), f0 + 1, nombre_columna(c1), f1 + 1)
    } else {
        format!("{}{}", nombre_columna(activa.columna), activa.fila + 1)
    };
    let ancho_ref = 150.0 * e;
    centrado_v(p, &referencia, f.x + 8.0 * e, f, tb, ancho_ref - 12.0 * e, TEXTO);
    p.rellenar(RectF { x: f.x + ancho_ref, y: f.y + 5.0 * e, ancho: 1.0, alto: f.alto - 10.0 * e }, LINEA);
    let x0 = f.x + ancho_ref + 10.0 * e;
    let modo = textos.t("hoja-editando");
    let ancho_modo = p.medir_texto(&modo, tb).0 + 20.0 * e;
    let una_linea = editor.en_la_barra().replace(['\r', '\n'], " ⏎ ");
    centrado_v(p, &una_linea, x0, f, tb, f.x + f.ancho - x0 - ancho_modo - 10.0 * e, TEXTO);
    if editor.edicion.is_some() {
        let w = p.medir_texto(&una_linea, tb).0;
        let cx = (x0 + w + 1.0).min(f.x + f.ancho - ancho_modo - 10.0 * e);
        p.rellenar(RectF { x: cx, y: f.y + 6.0 * e, ancho: (1.0 * e).max(1.0), alto: f.alto - 12.0 * e }, TEXTO);
    }
    // A la derecha, que se esta editando (en el verde de Excel).
    let caja_modo = RectF { x: f.x + f.ancho - ancho_modo, ancho: ancho_modo, ..f };
    centrado_v(p, &modo, caja_modo.x + 10.0 * e, caja_modo, tb, ancho_modo, VERDE);

    // Las pestanas de las hojas.
    let t = z.pestanas;
    p.rellenar(t, FONDO_BARRA);
    p.rellenar(RectF { alto: 1.0, ..t }, LINEA);
    v.pestanas.clear();
    let mut x = t.x + 8.0 * e;
    for (i, hoja) in editor.hojas.iter().enumerate() {
        let nombre = libro::nombre_de_pestana(&hoja.nombre);
        let es = i == editor.activa;
        let l = letra(es);
        let (w, th) = p.medir_con_letra(nombre, tam, pixpin_render::letras::SIN_PARTIR, &l);
        let caja = RectF { x, y: t.y, ancho: w + 24.0 * e, alto: t.alto };
        if caja.x > t.x + t.ancho {
            break;
        }
        if es {
            p.rellenar(caja, Color::BLANCO);
            p.rellenar(RectF { x: caja.x + 6.0 * e, y: caja.y + caja.alto - 3.0 * e, ancho: caja.ancho - 12.0 * e, alto: 2.0 * e }, VERDE);
        }
        p.texto_con_letra(nombre, caja.x + 12.0 * e, caja.y + (caja.alto - th) / 2.0, tam, pixpin_render::letras::SIN_PARTIR, &l, if es { VERDE } else { TEXTO });
        p.rellenar(RectF { x: caja.x + caja.ancho, y: caja.y + 6.0 * e, ancho: 1.0, alto: caja.alto - 12.0 * e }, LINEA);
        v.pestanas.push((caja, i));
        x += caja.ancho + 1.0;
    }

    if let Some((texto, _)) = &v.aviso {
        crate::lector::aviso_en(p, texto, v.ancho / 2.0, z.pestanas.y - 56.0 * e, e);
    }
}
