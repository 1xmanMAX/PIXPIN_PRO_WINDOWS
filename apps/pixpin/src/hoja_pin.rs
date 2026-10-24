//! **Las hojas del chat se abren como un pin** (el usuario, 11-oct-2026:
//! «que todo siempre se abra en version de pin pero en modo visualizacion, y
//! si tenemos algo como el editor de Excel, se pase a este con un boton
//! dentro de la misma interfaz del pin»).
//!
//! Un Excel adjunto (`.xlsx`, `.xlsm`, `.ods`, `.csv`, `.tsv`) o una tabla
//! creada en el chat se abren en el pin del visor de hojas (`visor_hoja`),
//! en modo ver, con «Editar» en su barra.
//!
//! **Editar tiene la misma cara que ver** (el usuario, 12-oct-2026: «el
//! editor de Excel y el visor deben tener la misma interfaz»): es el MISMO
//! visor, con su pintor (`pixpin_render::hoja_office`), su zoom, su
//! desplazamiento, sus barras, sus teclas y su raton; pasar de uno a otro no
//! mueve nada. Editar solo anade escribir en las casillas (doble clic, F2 o
//! empezar a teclear; Intro y Tab confirman, Esc cancela), las formulas, la
//! barra de formato (negrita, cursiva, subrayado, tachado, resaltar, color
//! de letra, alinear, formato de numero, ajustar texto, borrar formato) y
//! copiar, cortar, pegar, deshacer y rehacer. Este modulo pone lo que es de
//! editar: las teclas, los botones y lo que se escribe; el visor lo llama.
//!
//! - **Ver**: un `.xlsx` sin tocar se ve tal cual es (el original, con sus
//!   estilos, como desde el Explorador). Un `.csv`/`.ods`, una tabla del
//!   chat o un libro ya editado se ven con sus tablas pasadas a un `.xlsx`
//!   en memoria (`pixpin_proyecto::tabla_xlsx`), con el mismo pintor.
//! - **Editar**: las tablas del libro se leen la primera vez
//!   (`importar_hojas`) y no se releen. Mientras no se cambie nada se sigue
//!   viendo lo mismo; con el primer cambio lo que se ve pasa a ser las
//!   tablas (un `.xlsx` original pierde entonces lo que una tabla no guarda:
//!   bordes, letras propias...), y despues cada cambio rehace solo eso.
//! - **Se guarda en el chat**, en el mensaje-tabla de cada hoja
//!   (`libro-<mensaje>-<hoja>`, como el movil), asi la sincronizacion con
//!   Android sigue igual. El archivo original NO cambia, y se dice la
//!   primera vez. Un libro que solo se mira no deja tablas en el chat.
//! - **Un solo formato** (el usuario, 15-oct-2026: «las hojas editadas
//!   tienen UN SOLO formato, el del PC, y Android lee el del PC»): el JSON
//!   de `pixpin_proyecto::tabla::Tabla` (`TablaDeCalculo` en el movil),
//!   escrito por `Tabla::escribir` en todos los caminos (este editor, el
//!   clip, el asistente, pegar, importar). Lo viejo se lee y se migra al
//!   guardar: estilos-palabra (`Tabla::leer`) y las hojas que el movil
//!   guardaba solo en `tablas/libro-*.json` (`tablas_viejas_del_libro`).
//!   Guia para Android: `docs/manual-hojas-android.md`.
//! - Deshacer y rehacer (Ctrl+Z, Ctrl+Y) con una pila de cambios de celda
//!   (`editor.rs`).

pub mod editor;
pub mod libro;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_movil;

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
    /// Poner o quitar el modo de los marcos (como el boton del riel).
    pub const MARCOS: u8 = 5;
    /// Imprimir los marcos (Ctrl+P): las fotos quedan en
    /// `visor_hoja::marcos::FOTOS`, sin abrir el dialogo.
    pub const IMPRIMIR: u8 = 6;

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
        ORDENES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(orden);
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
        *VISTA.lock().unwrap_or_else(|e| e.into_inner()) = (Vec::new(), 1.0, [false; 3]);
    }

    /// Lo que se ve, para que la prueba sepa donde pulsar: el centro de
    /// cada ficha de los marcos (en la ventana), el zoom de la hoja y los
    /// modos del riel puestos (la tira del marcador, el lapiz y los marcos).
    static VISTA: Mutex<Vista> = Mutex::new((Vec::new(), 1.0, [false; 3]));
    pub type Vista = (Vec<(i32, i32)>, f64, [bool; 3]);

    pub fn apuntar_vista(v: Vista) {
        *VISTA.lock().unwrap_or_else(|e| e.into_inner()) = v;
    }

    pub fn vista() -> Vista {
        VISTA.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Las teclas que manda la prueba (la tecla y si va con Ctrl): el bucle
    /// las trata como pulsadas en su ventana, sin el teclado de verdad.
    static TECLAS: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());

    pub fn teclear(vk: u32, ctrl: bool) {
        TECLAS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((vk, ctrl));
        if let Some((h, ..)) = ventana() {
            pixpin_shell::overlay::despertar(h);
        }
    }

    pub fn tomar_teclas() -> Vec<(u32, bool)> {
        std::mem::take(&mut *TECLAS.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

use std::path::PathBuf;
use std::time::Instant;

use pixpin_hojas::{Libro, Regional};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::tabla::{FormatoNumero, Tabla};
use pixpin_render::icono::material;
use pixpin_render::letras::Letra;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::{Catalogo, Ubicacion};

use crate::lector_pin::BotonLector;
use editor::{Alineacion, Editor, Formato, HojaEditable, Interruptor};

// ---------------------------------------------------------------------------
// De donde viene la hoja

/// Que se abre al tocar una burbuja del chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    /// Un libro adjunto (Excel, OpenDocument, CSV).
    Libro,
    /// Una tabla creada en el chat (la mini-app `tabla`).
    Tabla,
    /// Una tabla hecha en el movil (`Clase.TABLA`): vive en su fichero
    /// `tablas/<referencia>.json`, no en el mensaje (12-oct-2026: antes el
    /// PC no la abria y no se podia cambiar aqui).
    TablaDelMovil,
}

/// **Si una burbuja se abre como pin de hoja**, y como. Un `.xls` antiguo no:
/// no se lee, y sigue su camino de siempre (su programa).
pub fn que_abrir(m: &Mensaje) -> Option<Que> {
    if m.clase == Some(Clase::MiniApp)
        && m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP)
    {
        return Some(Que::Tabla);
    }
    if m.clase == Some(Clase::Otra(pixpin_proyecto::vista::CLASE_TABLA.into()))
        && m.referencia
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty())
    {
        return Some(Que::TablaDelMovil);
    }
    (libro::es_libro(m) && libro::extension(m) != "xls").then_some(Que::Libro)
}

/// De donde salio la hoja que se ve.
pub(crate) enum Origen {
    /// Un libro adjunto: su mensaje, el fichero y las tablas que ya salieron
    /// de el (en el orden de sus hojas; vacio si nunca se edito).
    Libro {
        mensaje: Mensaje,
        ruta: PathBuf,
        tablas: Vec<Mensaje>,
    },
    /// Una tabla del chat.
    Tabla { mensaje: Mensaje },
    /// Una tabla del movil: su mensaje (con el JSON del fichero en `texto`,
    /// para leerla como las del chat) y el fichero, que es donde se guarda
    /// (`vista::tabla_del_movil`).
    TablaDelMovil { mensaje: Mensaje, archivo: PathBuf },
}

/// **Una hoja del chat abierta en el pin**: de donde viene y, desde que se
/// pulso «Editar», su editor (que se queda entre un modo y otro).
pub(crate) struct Editable {
    ubicacion: Ubicacion,
    proyecto: String,
    origen: Origen,
    editor: Option<Editor>,
    regional: Option<Regional>,
    /// Lo que se ve es el `.xlsx` original (las hojas se casan con las
    /// tablas por el nombre).
    vista_original: bool,
    /// Los cambios del editor que ya se ven ([`Editor::cambios`]).
    vistos: u64,
    /// El simbolo de la moneda del equipo (se lee una vez).
    moneda: Option<String>,
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
pub(crate) fn abrir_editando(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    proyecto: &str,
    mensajes: &[Mensaje],
    indice: usize,
) -> bool {
    abrir(idioma, ubicacion, proyecto, mensajes, indice, true)
}

fn abrir(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    proyecto: &str,
    mensajes: &[Mensaje],
    indice: usize,
    editando: bool,
) -> bool {
    let Some(m) = mensajes.get(indice) else {
        return false;
    };
    let origen = match que_abrir(m) {
        Some(Que::Tabla) => Origen::Tabla { mensaje: m.clone() },
        Some(Que::TablaDelMovil) => {
            let Some((mensaje, archivo)) = tabla_del_movil(ubicacion.raiz(), proyecto, m) else {
                return false;
            };
            Origen::TablaDelMovil { mensaje, archivo }
        }
        Some(Que::Libro) => {
            let Some(ruta) = m
                .ruta
                .as_deref()
                .and_then(|r| pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), proyecto, r))
                .filter(|r| r.is_file())
            else {
                return false;
            };
            let tablas = libro::tablas_hechas(mensajes, m)
                .into_iter()
                .map(|i| mensajes[i].clone())
                .collect();
            Origen::Libro {
                mensaje: m.clone(),
                ruta,
                tablas,
            }
        }
        None => return false,
    };
    tracing::info!(
        libro = matches!(origen, Origen::Libro { .. }),
        editando,
        "hoja del chat: se abre como pin"
    );
    let mut ed = Editable::nuevo(ubicacion.clone(), proyecto.to_string(), origen);
    ed.al_abrir_editar = editando;
    crate::visor_hoja::lanzar_editable(idioma, ed);
    true
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
            vistos: 0,
            moneda: None,
            avisado: false,
            al_abrir_editar: false,
        }
    }

    /// El archivo del libro, para «Abrir con su programa».
    pub(crate) fn ruta_original(&self) -> Option<PathBuf> {
        match &self.origen {
            Origen::Libro { ruta, .. } => Some(ruta.clone()),
            Origen::Tabla { .. } | Origen::TablaDelMovil { .. } => None,
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
            Origen::Tabla { mensaje } | Origen::TablaDelMovil { mensaje, .. } => {
                mensaje.nombre.clone()
            }
        };
        let n = crate::lector_pin::nombre_limpio(&n);
        if n.trim().is_empty() {
            "Tabla".into()
        } else {
            n
        }
    }

    /// Si lo que se ve es el `.xlsx` original (y no sus tablas).
    pub(crate) fn vista_original(&self) -> bool {
        self.vista_original
    }

    pub(crate) fn editor(&self) -> Option<&Editor> {
        self.editor.as_ref()
    }

    pub(crate) fn editor_mut(&mut self) -> Option<&mut Editor> {
        self.editor.as_mut()
    }

    /// **Lo que se ve en modo ver.** Un `.xlsx` que nadie edito, el
    /// original: se abre igual de rapido que desde el Explorador. Lo demas,
    /// sus tablas en un `.xlsx` hecho en memoria. Un libro con hojas viejas
    /// del movil (`tablas_viejas_del_libro`) y sin mensajes se ve por ellas:
    /// lo que se edito alli no se esconde.
    pub(crate) fn libro_para_ver(&mut self, regional: Regional) -> Result<Libro, String> {
        self.regional = Some(regional.clone());
        if let Origen::Libro {
            mensaje,
            ruta,
            tablas,
        } = &self.origen
            && crate::visor_hoja::se_abre_fichero(ruta)
            && !tablas.iter().any(libro::editada)
            && (!tablas.is_empty()
                || tablas_viejas_del_libro(self.ubicacion.raiz(), &self.proyecto, mensaje)
                    .is_none())
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
        self.vistos = self.editor.as_ref().map_or(0, Editor::cambios);
        self.libro_de_tablas()
    }

    /// Las tablas del editor pasadas a un libro para el visor.
    fn libro_de_tablas(&mut self) -> Result<Libro, String> {
        let moneda = self
            .moneda
            .get_or_insert_with(pixpin_shell::regional::simbolo_moneda)
            .clone();
        let ed = self.editor.as_ref().ok_or("sin tablas")?;
        let hojas: Vec<pixpin_proyecto::tabla_xlsx::HojaParaVer> = ed
            .hojas
            .iter()
            .map(|h| pixpin_proyecto::tabla_xlsx::HojaParaVer {
                nombre: libro::nombre_de_pestana(&h.nombre),
                tabla: &h.tabla,
                valores: Some(h.valores()),
            })
            .collect();
        let t = Instant::now();
        let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx_con_moneda(&hojas, ed.activa, &moneda)
            .map_err(|e| e.to_string())?;
        let regional = self.regional.clone().unwrap_or_else(Regional::es_pe);
        let l = Libro::desde_bytes(bytes, regional).map_err(|e| e.to_string())?;
        tracing::info!(
            ms = t.elapsed().as_millis() as u64,
            hojas = hojas.len(),
            "hoja del chat: tablas pasadas al visor"
        );
        Ok(l)
    }

    /// **Lo que se ve, al dia**: si el editor cambio algo desde la ultima
    /// vez, el libro nuevo con sus tablas (lo que se ve deja de ser el
    /// original). `None` si no hay nada nuevo.
    pub(crate) fn libro_al_dia(&mut self) -> Option<Libro> {
        let cambios = self.editor.as_ref()?.cambios();
        if cambios == self.vistos {
            return None;
        }
        self.vistos = cambios;
        match self.libro_de_tablas() {
            Ok(l) => {
                self.vista_original = false;
                Some(l)
            }
            Err(e) => {
                tracing::warn!(%e, "hoja del chat: no se pudo rehacer lo que se ve");
                None
            }
        }
    }

    /// **Monta el editor** si no lo estaba: las tablas que ya hay en el chat
    /// o, la primera vez, el libro leido como tablas. No se relee nunca.
    pub(crate) fn montar_editor(&mut self) -> Result<(), String> {
        if self.editor.is_some() {
            return Ok(());
        }
        let t = Instant::now();
        let hojas = match &self.origen {
            Origen::Tabla { mensaje } | Origen::TablaDelMovil { mensaje, .. } => {
                let tabla = Tabla::leer(&mensaje.texto)
                    .map_err(|e| format!("tabla que no se entiende: {e}"))?;
                let nombre = if tabla.nombre.trim().is_empty() {
                    mensaje.nombre.clone()
                } else {
                    tabla.nombre.clone()
                };
                vec![HojaEditable::nueva(nombre, tabla, Some(mensaje.id.clone()))]
            }
            Origen::Libro { tablas, .. } if !tablas.is_empty() => tablas
                .iter()
                .map(|m| {
                    let tabla = Tabla::leer(&m.texto)
                        .map_err(|e| format!("tabla que no se entiende: {e}"))?;
                    Ok(HojaEditable::nueva(
                        m.nombre.clone(),
                        tabla,
                        Some(m.id.clone()),
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?,
            Origen::Libro { mensaje, ruta, .. } => {
                if let Some(viejas) =
                    tablas_viejas_del_libro(self.ubicacion.raiz(), &self.proyecto, mensaje)
                {
                    // Lo que el movil edito antes del 13-oct (solo en
                    // fichero): se edita eso, no el libro leido de nuevo, y
                    // al guardar pasa a mensajes como cualquier libro editado.
                    tracing::info!(
                        hojas = viejas.len(),
                        "hoja del chat: se migran las hojas viejas del movil"
                    );
                    viejas
                        .into_iter()
                        .map(|(nombre, t)| HojaEditable::nueva(nombre, t, None))
                        .collect()
                } else {
                    let ahora = pixpin_shell::entorno::ahora_utc_ms();
                    pixpin_proyecto::importar_hojas::leer(ruta, &mensaje.nombre, ahora)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|h| HojaEditable::nueva(h.nombre, h.tabla, None))
                        .collect()
                }
            }
        };
        tracing::info!(
            ms = t.elapsed().as_millis() as u64,
            hojas = hojas.len(),
            "hoja del chat: tablas listas para editar"
        );
        self.editor = Some(Editor::nuevo(hojas, 0).ok_or("el libro no tiene hojas")?);
        Ok(())
    }

    /// **La tabla que se edita viendo la hoja `vista`** (de las que se ven,
    /// `nombres_vista`): la misma si se ven las tablas; la de su nombre si
    /// se ve el original. `None`: esa hoja no tiene tabla (una de graficos).
    pub(crate) fn tabla_de_vista(&self, nombres_vista: &[&str], vista: usize) -> Option<usize> {
        let ed = self.editor.as_ref()?;
        let nombres: Vec<&str> = ed.hojas.iter().map(|h| h.nombre.as_str()).collect();
        tabla_de_vista(self.vista_original, nombres_vista, &nombres, vista)
    }

    /// El aviso de que el archivo original no cambia, la primera vez que se
    /// edita un libro (despues, nada).
    pub(crate) fn aviso_al_editar(&mut self) -> Option<&'static str> {
        if self.avisado || !self.primera_edicion_de_libro() {
            return None;
        }
        self.avisado = true;
        Some("hoja-editar-aviso")
    }

    /// Si editar este libro deja sus tablas nuevas en el chat (la primera
    /// vez): es cuando hay que decir que el original no cambia.
    fn primera_edicion_de_libro(&self) -> bool {
        matches!(self.origen, Origen::Libro { .. })
            && self
                .editor
                .as_ref()
                .is_some_and(|e| e.hojas.iter().any(|h| h.id_mensaje.is_none()))
    }

    /// **Guarda lo cambiado en el chat**: cada tabla tocada en su mensaje
    /// (`cuaderno::cambiar`, con el cerrojo del cuaderno) y, si el libro
    /// aun no tenia tablas, todas sus hojas como mensajes nuevos con la
    /// referencia del libro. Despues avisa al chat para que lo relea.
    pub(crate) fn guardar(&mut self) -> Result<(), String> {
        let Some(ed) = self.editor.as_mut() else {
            return Ok(());
        };
        ed.confirmar();
        if !ed.tocado() {
            return Ok(());
        }
        let raiz = self.ubicacion.raiz();
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &self.proyecto);
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let nuevas = ed.hojas.iter().any(|h| h.id_mensaje.is_none());
        if nuevas
            && let Origen::Libro {
                mensaje, tablas, ..
            } = &mut self.origen
        {
            let numero = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
                .map(|c| c.siguiente_numero(Some(&self.proyecto)))
                .unwrap_or(1);
            let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC")
                .map(|i| i.yo.codigo())
                .unwrap_or_default();
            let sello = pixpin_proyecto::cuaderno::Sello {
                cuando: ahora,
                numero,
                aparato,
                proyecto: self.proyecto.clone(),
            };
            // La hoja cambiada lleva un `tocado` posterior a la hora de su
            // mensaje (`libro::editada`); las demas, la de leerla.
            for (i, h) in ed.hojas.iter_mut().enumerate().filter(|(_, h)| h.tocada) {
                h.tabla.tocado = editada_a(ahora, sello.cuando + i as i64);
            }
            let listas = ed
                .hojas
                .iter()
                .map(|h| {
                    Ok(libro::HojaLista {
                        nombre: h.nombre.clone(),
                        documento: h.tabla.escribir().map_err(|e| e.to_string())?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let nuevos = libro::mensajes_de_hojas(mensaje, &listas, &sello);
            let lineas = nuevos
                .iter()
                .map(pixpin_proyecto::cuaderno::linea_de)
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            pixpin_proyecto::cuaderno::anadir_lineas(&carpeta, &lineas)
                .map_err(|e| e.to_string())?;
            for (h, m) in ed.hojas.iter_mut().zip(&nuevos) {
                h.id_mensaje = Some(m.id.clone());
                h.tocada = false;
            }
            *tablas = nuevos;
            tracing::info!(
                hojas = ed.hojas.len(),
                "hoja del chat: el libro editado queda como tablas del chat"
            );
        } else if let Origen::TablaDelMovil { archivo, .. } = &self.origen {
            // Una tabla del movil se guarda en SU fichero, el que viaja.
            for h in ed.hojas.iter_mut().filter(|h| h.tocada) {
                let texto = tabla_del_movil_al_guardar(&mut h.tabla, ahora)?;
                pixpin_proyecto::vista::guardar_tabla_del_movil(archivo, &texto)
                    .map_err(|e| e.to_string())?;
                h.tocada = false;
            }
        } else {
            for h in ed.hojas.iter_mut().filter(|h| h.tocada) {
                let Some(id) = h.id_mensaje.clone() else {
                    continue;
                };
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
                    Ok(None) => tracing::warn!(
                        "hoja del chat: la tabla ya no esta en el chat; no se guarda"
                    ),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
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

/// **Una tabla del movil, lista para abrirla**: su mensaje con el JSON de su
/// fichero en `texto` (asi se lee como cualquier tabla del chat) y el
/// fichero. `None` si el fichero aun no llego o no se lee: mejor no abrir
/// que abrir en blanco y guardar encima.
pub(crate) fn tabla_del_movil(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &Mensaje,
) -> Option<(Mensaje, PathBuf)> {
    let archivo = pixpin_proyecto::vista::tabla_del_movil(raiz, proyecto, m)?;
    let texto = std::fs::read_to_string(&archivo).ok()?;
    Tabla::leer(&texto).ok()?;
    let mut mensaje = m.clone();
    mensaje.texto = texto;
    Some((mensaje, archivo))
}

/// **Las hojas viejas del movil de un libro** (`tablas/libro-<id>-<hoja>.json`,
/// ver `vista::tablas_viejas_del_libro`), con su nombre de pestana. Se leen
/// hasta la primera que no se entienda (como `TablasDelChat.crearLasDelLibro`
/// del movil); si ni la primera se entiende, `None` y se lee el libro. Los
/// ficheros no se tocan: uno roto sigue ahi tal cual.
pub(crate) fn tablas_viejas_del_libro(
    raiz: &std::path::Path,
    proyecto: &str,
    libro_m: &Mensaje,
) -> Option<Vec<(String, Tabla)>> {
    let mut hojas = Vec::new();
    for (i, archivo) in pixpin_proyecto::vista::tablas_viejas_del_libro(raiz, proyecto, &libro_m.id)
        .into_iter()
        .enumerate()
    {
        let leida = std::fs::read_to_string(&archivo)
            .map_err(|e| e.to_string())
            .and_then(|t| Tabla::leer(&t).map_err(|e| e.to_string()));
        match leida {
            Ok(t) => {
                let nombre = if t.nombre.trim().is_empty() {
                    format!("Hoja{}", i + 1)
                } else {
                    t.nombre.clone()
                };
                hojas.push((nombre, t));
            }
            Err(e) => {
                tracing::warn!(%e, hoja = i, "hoja vieja del movil que no se entiende; se deja como esta");
                break;
            }
        }
    }
    (!hojas.is_empty()).then_some(hojas)
}

/// El JSON con que se guarda una tabla del movil, con su `tocado` a la hora
/// de guardar (`TablaDeCalculo.tocado`, lo que su lista ensena).
fn tabla_del_movil_al_guardar(tabla: &mut Tabla, ahora: i64) -> Result<String, String> {
    tabla.tocado = tabla.tocado.max(ahora);
    tabla.escribir().map_err(|e| e.to_string())
}

/// **Que tabla es la hoja `vista`** de lo que se ve: viendo las tablas, la
/// misma; viendo el original, la que se llama como ella («libro · hoja»,
/// como las llama `importar_hojas` cuando hay varias) o, si el libro solo
/// dio una tabla, esa. Una hoja de graficos o sin datos de un libro con
/// varias tablas no tiene tabla: `None` (no se edita).
pub fn tabla_de_vista(
    vista_original: bool,
    nombres_vista: &[&str],
    nombres_tablas: &[&str],
    vista: usize,
) -> Option<usize> {
    if !vista_original {
        return (vista < nombres_tablas.len()).then_some(vista);
    }
    let nombre = nombres_vista.get(vista)?;
    if nombres_tablas.len() == 1 {
        return Some(0);
    }
    nombres_tablas
        .iter()
        .position(|t| libro::nombre_de_pestana(t) == *nombre)
}

/// Lo que escribe la orden `ESCRIBIR` de las pruebas (`prueba`).
#[cfg(test)]
pub(crate) fn escribir_muestra(ed: &mut Editable) {
    if let Some(e) = ed.editor_mut() {
        pruebas::escribir_muestra(e);
    }
}

// ---------------------------------------------------------------------------
// Las teclas del modo editar

const VK_RETROCESO: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_REPAG: u32 = 0x21;
const VK_AVPAG: u32 = 0x22;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ABAJO: u32 = 0x28;
const VK_SUPR: u32 = 0x2E;
const VK_5: u32 = 0x35;
const VK_F2: u32 = 0x71;

/// Lo que hace una tecla en modo editar (lo que no esta aqui lo hace el
/// visor: moverse, el zoom, las hojas...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccionEd {
    /// Escribiendo: dejar escrito y que el visor haga lo de la tecla
    /// (Intro baja, Tab va a la derecha, las flechas mueven).
    Confirmar,
    /// F2: escribir en la celda con lo que tiene.
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
    Formato(Formato),
    /// Una tecla que escribiendo no hace nada (Ctrl+A).
    Nada,
}

/// **Que hace una tecla en modo editar**, sabiendo si se esta escribiendo en
/// una celda. Las letras sueltas no estan: llegan como caracteres. Los
/// atajos de formato, los de siempre en las dos Excel (espanola e inglesa):
/// Ctrl+N/B negrita, Ctrl+K/I cursiva, Ctrl+S/U subrayado, Ctrl+5 tachado
/// y Ctrl+Mayus+L/E/R alinear (como Google Sheets).
pub fn accion_de_tecla(vk: u32, shift: bool, ctrl: bool, escribiendo: bool) -> Option<AccionEd> {
    use AccionEd::*;
    let alternar = |i| Some(Formato(self::Formato::Alternar(i)));
    if ctrl {
        return match vk {
            0x5A if shift => Some(Rehacer),
            0x5A => Some(Deshacer),
            0x59 => Some(Rehacer),
            0x43 => Some(Copiar),
            0x58 => Some(Cortar),
            0x56 => Some(Pegar),
            0x4C if shift => Some(Formato(self::Formato::Alinear(Alineacion::Izquierda))),
            0x45 if shift => Some(Formato(self::Formato::Alinear(Alineacion::Centro))),
            0x52 if shift => Some(Formato(self::Formato::Alinear(Alineacion::Derecha))),
            0x42 | 0x4E => alternar(Interruptor::Negrita),
            0x4B | 0x49 => alternar(Interruptor::Cursiva),
            0x53 | 0x55 => alternar(Interruptor::Subrayado),
            VK_5 => alternar(Interruptor::Tachado),
            0x41 if escribiendo => Some(Nada),
            // Ctrl+Inicio/Fin/flechas escribiendo: se deja lo escrito y se
            // mueve el visor.
            VK_INICIO | VK_FIN | VK_IZQUIERDA..=VK_ABAJO if escribiendo => Some(Confirmar),
            _ => None,
        };
    }
    match vk {
        VK_ESCAPE if escribiendo => Some(Cancelar),
        VK_ESCAPE => Some(Ver),
        VK_ENTRAR | VK_TAB | VK_REPAG | VK_AVPAG | VK_IZQUIERDA..=VK_ABAJO if escribiendo => {
            Some(Confirmar)
        }
        VK_F2 => Some(Empezar),
        VK_RETROCESO => Some(Retroceso),
        VK_SUPR if !escribiendo => Some(Borrar),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// La barra de formato

/// Botones propios de la barra en modo editar (los del modo ver son 1-3).
pub(crate) const BOTON_VER: u8 = 10;
const BOTON_DESHACER: u8 = 11;
const BOTON_REHACER: u8 = 12;
const BOTON_NEGRITA: u8 = 13;
const BOTON_CURSIVA: u8 = 14;
const BOTON_SUBRAYADO: u8 = 15;
const BOTON_TACHADO: u8 = 16;
const BOTON_RESALTAR: u8 = 17;
const BOTON_LETRA: u8 = 18;
const BOTON_ALINEAR: u8 = 19;
const BOTON_NUMERO: u8 = 20;
const BOTON_AJUSTAR: u8 = 21;
const BOTON_BORRAR_FORMATO: u8 = 22;

/// **El nombre y el atajo de cada boton al editar** (19-oct-2026, el
/// usuario: «que al pasar por un icono me aparezca su atajo»), para el
/// globo de la barra. Las teclas son las de [`accion_de_tecla`].
pub(crate) fn pistas_de_editar(t: &pixpin_store::Catalogo) -> Vec<(u8, String)> {
    [
        (BOTON_VER, "hoja-pista-ver"),
        (BOTON_DESHACER, "barra-pista-deshacer"),
        (BOTON_REHACER, "barra-pista-rehacer"),
        (BOTON_NEGRITA, "hoja-pista-negrita"),
        (BOTON_CURSIVA, "hoja-pista-cursiva"),
        (BOTON_SUBRAYADO, "hoja-pista-subrayado"),
        (BOTON_TACHADO, "hoja-pista-tachado"),
        (BOTON_RESALTAR, "hoja-pista-resaltar"),
        (BOTON_LETRA, "hoja-pista-letra"),
        (BOTON_ALINEAR, "hoja-pista-alinear"),
        (BOTON_NUMERO, "hoja-pista-numero"),
        (BOTON_AJUSTAR, "hoja-pista-ajustar"),
        (BOTON_BORRAR_FORMATO, "hoja-pista-borrar-formato"),
    ]
    .into_iter()
    .map(|(id, clave)| (id, t.t(clave)))
    .collect()
}

/// **Los botones de la barra al editar**: volver a ver, deshacer, rehacer y
/// el formato. Los interruptores se encienden si la celda activa los tiene,
/// y el de alinear ensena la alineacion que tiene (como el movil).
pub(crate) fn botones_de_editar(ed: &Editor) -> Vec<BotonLector> {
    let e = ed.hoja().estilo_activo().cloned().unwrap_or_default();
    let b = |id, icono, encendido| BotonLector {
        id,
        icono,
        encendido,
    };
    let alinear = match e.a.as_deref().and_then(Alineacion::de) {
        Some(Alineacion::Centro) => &material::FORMAT_ALIGN_CENTER,
        Some(Alineacion::Derecha) => &material::FORMAT_ALIGN_RIGHT,
        _ => &material::FORMAT_ALIGN_LEFT,
    };
    vec![
        b(BOTON_VER, &material::VISIBILITY, false),
        b(BOTON_DESHACER, &material::UNDO, false),
        b(BOTON_REHACER, &material::REDO, false),
        b(BOTON_NEGRITA, &material::FORMAT_BOLD, e.n),
        b(BOTON_CURSIVA, &material::FORMAT_ITALIC, e.k),
        b(BOTON_SUBRAYADO, &material::FORMAT_UNDERLINED, e.s),
        b(BOTON_TACHADO, &material::FORMAT_STRIKETHROUGH, e.t),
        b(BOTON_RESALTAR, &material::FORMAT_COLOR_FILL, e.f.is_some()),
        b(BOTON_LETRA, &material::FORMAT_COLOR_TEXT, e.c.is_some()),
        b(BOTON_ALINEAR, alinear, e.a.is_some()),
        b(BOTON_NUMERO, &material::NUMBERS, e.nf.is_some()),
        b(BOTON_AJUSTAR, &material::WRAP_TEXT, e.w),
        b(BOTON_BORRAR_FORMATO, &material::FORMAT_CLEAR, false),
    ]
}

/// Lo que pide un boton de la barra al editar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PideBoton {
    Ver,
    Deshacer,
    Rehacer,
    Formato(Formato),
    /// Elegir en un menu (los colores y el formato de numero).
    Menu(Menu),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    Resaltar,
    Letra,
    Numero,
}

/// Que hace el boton `id` (`None`: no es de editar).
pub(crate) fn pide_boton(id: u8) -> Option<PideBoton> {
    use PideBoton as P;
    let alternar = |i| Some(P::Formato(Formato::Alternar(i)));
    match id {
        BOTON_VER => Some(P::Ver),
        BOTON_DESHACER => Some(P::Deshacer),
        BOTON_REHACER => Some(P::Rehacer),
        BOTON_NEGRITA => alternar(Interruptor::Negrita),
        BOTON_CURSIVA => alternar(Interruptor::Cursiva),
        BOTON_SUBRAYADO => alternar(Interruptor::Subrayado),
        BOTON_TACHADO => alternar(Interruptor::Tachado),
        BOTON_AJUSTAR => alternar(Interruptor::Ajustar),
        BOTON_ALINEAR => Some(P::Formato(Formato::AlinearSiguiente)),
        BOTON_BORRAR_FORMATO => Some(P::Formato(Formato::Borrar)),
        BOTON_RESALTAR => Some(P::Menu(Menu::Resaltar)),
        BOTON_LETRA => Some(P::Menu(Menu::Letra)),
        BOTON_NUMERO => Some(P::Menu(Menu::Numero)),
        _ => None,
    }
}

/// **Las entradas de un menu** (id, rotulo), con una marca en lo que tiene
/// la celda activa; y el formato que da cada id.
pub(crate) fn entradas_de_menu(
    menu: Menu,
    ed: &Editor,
    textos: &Catalogo,
) -> Vec<(u32, String, Formato)> {
    let e = ed.hoja().estilo_activo().cloned().unwrap_or_default();
    let marca = |si: bool, t: String| if si { format!("\u{2713} {t}") } else { t };
    match menu {
        Menu::Resaltar | Menu::Letra => {
            let (lista, actual, ninguno) = match menu {
                Menu::Resaltar => (&editor::FONDOS[..], e.f.clone(), "hoja-fondo-sin"),
                _ => (&editor::LETRAS[..], e.c.clone(), "hoja-letra-automatica"),
            };
            let poner = |c: Option<String>| {
                if menu == Menu::Resaltar {
                    Formato::Fondo(c)
                } else {
                    Formato::Letra(c)
                }
            };
            let mut v: Vec<(u32, String, Formato)> = lista
                .iter()
                .enumerate()
                .map(|(i, (hex, clave))| {
                    (
                        i as u32 + 1,
                        marca(actual.as_deref() == Some(*hex), textos.t(clave)),
                        poner(Some(hex.to_string())),
                    )
                })
                .collect();
            v.push((
                lista.len() as u32 + 1,
                marca(actual.is_none(), textos.t(ninguno)),
                poner(None),
            ));
            v
        }
        Menu::Numero => {
            let actual = e.formato_numero();
            let mut v = vec![(
                1,
                marca(actual.is_none(), textos.t("hoja-numero-general")),
                Formato::Numero(None),
            )];
            for (i, f) in FormatoNumero::TODOS.into_iter().enumerate() {
                let clave = format!("hoja-numero-{}", f.palabra());
                v.push((
                    i as u32 + 2,
                    marca(actual == Some(f), textos.t(&clave)),
                    Formato::Numero(Some(f)),
                ));
            }
            v
        }
    }
}

// ---------------------------------------------------------------------------
// Copiar y pegar

/// Copia lo elegido: los valores con tabuladores y las formulas en la tabla
/// HTML (para pegar en Excel o Sheets). Escribiendo, lo escrito. Devuelve
/// el aviso que hay que ensenar.
pub(crate) fn copiar(editor: &Editor, textos: &Catalogo) -> Option<String> {
    let hecho = match &editor.edicion {
        Some(t) => pixpin_codec::portapapeles::copiar_texto(t),
        None => {
            let h = editor.hoja();
            if h.sel.celdas() > editor::MAXIMO_RANGO {
                return Some(textos.t("hoja-rango-grande"));
            }
            let decimal = pixpin_shell::entorno::separador_decimal();
            let (tsv, html) = pixpin_proyecto::portapapeles_tabla::copiar(
                &h.tabla,
                h.sel.ancla,
                h.sel.activa,
                decimal,
            );
            pixpin_codec::portapapeles::tabla::copiar_tabla(&tsv, &html)
        }
    };
    match hecho {
        Ok(()) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("celdas", editor.hoja().sel.celdas());
            Some(textos.t_args("hoja-copiado", &args))
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar de la hoja");
            None
        }
    }
}

/// Pega desde Excel, Sheets o LibreOffice (o texto). Escribiendo, el texto
/// entra en lo que se escribe, en una linea.
pub(crate) fn pegar(editor: &mut Editor) {
    let Some((html, texto)) = pixpin_codec::portapapeles::tabla::leer_tabla() else {
        return;
    };
    if let Some(edicion) = editor.edicion.as_mut() {
        if let Some(t) = texto {
            edicion.push_str(t.replace(['\r', '\n', '\t'], " ").trim_end());
        }
        return;
    }
    let Some(filas) = pixpin_proyecto::portapapeles_tabla::leer(html.as_deref(), texto.as_deref())
    else {
        return;
    };
    let hecho = editor.pegar(&filas);
    tracing::info!(filas = filas.len(), hecho, "pegado en la hoja del pin");
}

// ---------------------------------------------------------------------------
// Pintar lo que se escribe

const TEXTO: Color = Color {
    r: 0.13,
    g: 0.13,
    b: 0.13,
    a: 1.0,
};
const VERDE: Color = Color {
    r: 0.129,
    g: 0.451,
    b: 0.275,
    a: 1.0,
};

/// **La celda que se escribe**, encima de la hoja: en blanco, con lo
/// tecleado en la letra de la hoja al zoom que se ve (Calibri 11) y el
/// cursor al final. `caja` es la de la celda en pantalla; si lo escrito no
/// cabe, la caja crece hacia la derecha, como en Excel.
pub(crate) fn pintar_escritura(p: &Pintor, texto: &str, caja: [f32; 4], tam: f32, recorte: RectF) {
    let l = Letra {
        familia: "Calibri",
        negrita: false,
        cursiva: false,
        interlineado: None,
    };
    let (w, th) = p.medir_con_letra(texto, tam, pixpin_render::letras::SIN_PARTIR, &l);
    let relleno = (tam * 0.27).max(2.0);
    let celda = RectF {
        x: caja[0],
        y: caja[1],
        ancho: caja[2] - caja[0],
        alto: caja[3] - caja[1],
    };
    let ancho = (w + 2.0 * relleno + 2.0).max(celda.ancho);
    let r = RectF { ancho, ..celda };
    p.empujar_recorte(recorte);
    // Con el modo oscuro de los visores, la luz invertida como el resto.
    let oscuro = crate::oscuro::activo();
    let k = |c: Color| pixpin_render::oscuro::si(oscuro, c);
    p.rellenar(r, k(Color::BLANCO));
    let y = r.y + (r.alto - th) / 2.0;
    p.texto_con_letra(
        texto,
        r.x + relleno,
        y,
        tam,
        pixpin_render::letras::SIN_PARTIR,
        &l,
        k(TEXTO),
    );
    p.rellenar(
        RectF {
            x: r.x + relleno + w + 1.0,
            y: y + 1.0,
            ancho: (tam / 14.0).max(1.0),
            alto: (th - 2.0).max(1.0),
        },
        k(TEXTO),
    );
    p.trazar(r, (tam / 7.0).round().max(2.0), k(VERDE));
    p.soltar_recorte();
}
