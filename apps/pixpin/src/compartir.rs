//! **Compartir cualquier cosa** (G1, G3, D8): la hoja de compartir del
//! movil, una sola para toda la aplicacion.
//!
//! Es `ui/HojaDeCompartir.kt` + `ui/CompartirPaginas.kt` del movil. Alli todo
//! lo compartible cabe en «unas paginas de unos proyectos», y por eso los
//! generadores viven una sola vez y los usan todos: el mismo PDF, la misma
//! imagen y la misma pagina web salgan de donde salgan. Aqui igual, y un
//! paso mas: **todo lo que se comparte se convierte primero en hojas del
//! motor** (`pixpin_motor2d::exportar::Hoja`, lo mismo que exporta el
//! lienzo). Un lienzo y sus marcos, una foto con lo dibujado encima, una
//! pagina de PDF con lo anotado, un Word o un libro con su tinta, una nota,
//! una tabla, una mini-app: cada cosa pone sus hojas, y de hojas ya saben
//! salir la pagina web con los mandos del movil (`exportar_html`), el PDF
//! (`pixpin_pdf::escribir`), el SVG (`exportar_svg`) y el PNG/JPG (el
//! `Pintor` de la pantalla, `ventana_editor::exportar::a_imagen`). Nada de
//! eso se reescribe aqui: esto solo reune las hojas.
//!
//! Lo que no es hojas va **entero** (`Compartible.NINGUNA` del movil): el
//! fichero original, el `.pixpin` del proyecto, el `.excalidraw` del lienzo,
//! el texto de una nota o el CSV de una tabla.
//!
//! La ventana de la hoja, con sus salidas (el panel Compartir de Windows,
//! «Guardar como», copiar y la wifi), esta en [`ventana`]; la logica de que
//! formatos se ven y donde cae cada mando, en `pixpin_ui::hoja_compartir`.

pub(crate) mod adjuntos;
pub(crate) mod enlace;
pub(crate) mod documento_web;
mod pdf;
pub(crate) mod pdf_anotado;
pub(crate) mod ventana;
/// Una nota como Word (H12, 1-oct).
mod word;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_motor2d::exportar::{self as ex, Alcance, Hoja, ID_PAPEL};
use pixpin_motor2d::exportar_html::{self, HojaWeb};
use pixpin_motor2d::exportar_svg::{self, Incrustada, OpcionesSvg};
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Escena, EstiloTrazo};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::Catalogo;
use pixpin_ui::hoja_compartir::{Compartible, Cuantas, Formato, Pagina};

// ---------------------------------------------------------------------------
// Lo que se comparte

/// Un lienzo que ya esta abierto (el del editor): su escena y los pixeles de
/// sus fotos y de su papel, copiados para que la hoja viva en su hilo.
pub(crate) struct LienzoSuelto {
    pub escena: Escena,
    /// El papel de fondo y su tamano en la escena.
    pub papel: Option<(ImagenRgba, f32, f32)>,
    pub fotos: HashMap<u64, ImagenRgba>,
    pub nombre: String,
}

/// **Lo que se comparte.** Cada sitio que ofrece «Compartir…» dice que es, y
/// de aqui en adelante todo va igual.
// Una sola por ventana de compartir y se consume enseguida; meterla en Box no gana nada.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Cosa {
    /// El lienzo del editor.
    Lienzo(LienzoSuelto),
    /// Uno o varios mensajes de un proyecto del chat: la burbuja o lo
    /// marcado.
    Mensajes {
        raiz: PathBuf,
        proyecto: String,
        titulo: String,
        mensajes: Vec<Mensaje>,
    },
    /// Uno o varios proyectos enteros de la lista.
    Proyectos { raiz: PathBuf, ids: Vec<String> },
    /// Un documento que se esta leyendo (PDF, Word, libro...), con lo
    /// anotado que tenga al lado.
    Documento(PathBuf),
    /// **Algo ya hecho fuera de la hoja** (el timeline, 8-oct-2026): sus
    /// formatos enteros, sin paginas que elegir. La hoja hace lo demas: el
    /// peso, Guardar, Copiar y la Salida para arrastrarlo.
    Hecho { titulo: String, extras: Vec<Extra> },
}

/// El idioma de la aplicacion, leido de sus ajustes: el editor y los
/// lectores no lo reciben, y la hoja lo necesita para hablar como el resto.
pub(crate) fn idioma_de(ubicacion: &pixpin_store::Ubicacion) -> pixpin_store::Idioma {
    let ajustes = pixpin_store::cargar(ubicacion).unwrap_or_default();
    pixpin_store::resolver_idioma(&pixpin_shell::entorno::locale_del_sistema(), ajustes.idioma)
}

// Los formatos, por el nombre que entiende `generar`.
pub(crate) const WEB: &str = "web";
pub(crate) const PDF: &str = "pdf";
/// El PDF de un documento con el interruptor «Con anotaciones» quitado.
pub(crate) const PDF_LIMPIO: &str = "pdf-limpio";
/// La pagina web de un PDF con el interruptor «Texto buscable» quitado:
/// todas las hojas como imagen.
pub(crate) const WEB_IMAGEN: &str = "web-imagen";
pub(crate) const PNG: &str = "png";
pub(crate) const JPG: &str = "jpg";
pub(crate) const SVG: &str = "svg";

/// De donde salen los pixeles de una imagen de las hojas. Se leen al
/// generar y no al preparar: un proyecto con doscientas paginas de PDF no
/// puede tener doscientas paginas dibujadas en memoria para abrir la hoja.
#[derive(Clone)]
pub(crate) enum FuenteImagen {
    Cargada(Arc<ImagenRgba>),
    Fichero(PathBuf),
    PaginaPdf {
        pdf: PathBuf,
        pagina: u32,
        ancho: u32,
    },
}

/// Una pagina de lo que se comparte: la fila de la hoja y lo que se pinta.
pub(crate) struct Pieza {
    pub pagina: Pagina,
    pub hoja: Hoja,
    /// El papel. Casi siempre blanco; un Word se lee sobre papel oscuro y
    /// su tinta es clara, asi que sale oscuro.
    pub fondo: ColorRgba,
    /// La hoja del lienzo entero cuando tiene marcos: la pagina web los
    /// ensena para imprimir marco a marco.
    pub con_marcos: bool,
}

/// Lo que va entero, sin paginas.
pub(crate) enum Entero {
    /// Ficheros que ya existen, tal cual (los originales).
    Ficheros(Vec<PathBuf>),
    /// Un fichero que se escribe con estos bytes.
    Escrito { fichero: String, bytes: Vec<u8> },
    /// Un PDF impreso por Edge desde una pagina web (la del timeline): se
    /// imprime al pedirlo, porque tarda un par de segundos.
    PdfDeHtml { fichero: String, html: String },
    /// El `.pixpin` de unos proyectos `(raiz, id, fichero)`, que se empaqueta
    /// al pedirlo: puede pesar
    /// mucho y no hay por que hacerlo si no se elige.
    Proyectos(Vec<(PathBuf, String, String)>),
}

pub(crate) struct Extra {
    pub id: &'static str,
    /// La clave del nombre en el catalogo.
    pub clave: &'static str,
    pub que: Entero,
}

/// **Lo compartible, ya preparado**: las paginas, de donde salen sus
/// imagenes y lo que va entero.
pub(crate) struct Preparado {
    pub titulo: String,
    pub piezas: Vec<Pieza>,
    pub imagenes: HashMap<u64, FuenteImagen>,
    pub extras: Vec<Extra>,
    /// El lienzo que viaja dentro de la pagina web para volver a editarlo:
    /// solo cuando lo compartido es UN lienzo.
    pub excalidraw: Option<String>,
    /// Lo marcado al abrir; `None`, todo.
    pub marcadas: Option<BTreeSet<String>>,
    /// Si lo natural es mandar el original (una foto, un PDF, un Word): va
    /// el primero de la fila.
    pub original_primero: bool,
    /// **Los documentos que se leen** (un Word, un libro, un PDF) con la
    /// clave de sus filas: su «Pagina web» no son dibujos de sus paginas
    /// sino el documento de verdad con lo anotado y los marcadores, en el
    /// que se sigue anotando (ver [`documento_web`]).
    pub documentos: Vec<(String, PathBuf)>,
    /// **Las tablas**, por la clave de su fila: su «Pagina web» no es el
    /// dibujo de la rejilla sino la tabla que sigue calculando en el
    /// navegador (J3, `pixpin_proyecto::tabla_web`).
    pub tablas: HashMap<String, pixpin_proyecto::tabla::Tabla>,
    /// **Lo que va dentro de la pagina web sin ser una hoja**: las notas de
    /// voz y los demas ficheros de lo compartido (ver [`adjuntos`]).
    pub adjuntos: Vec<PathBuf>,
    /// «Adjuntos», en el idioma de la app: la pagina no sabe de idiomas.
    pub rotulo_adjuntos: String,
    siguiente_imagen: u64,
}

impl Preparado {
    fn nuevo(titulo: impl Into<String>) -> Preparado {
        Preparado {
            titulo: titulo.into(),
            piezas: Vec::new(),
            imagenes: HashMap::new(),
            extras: Vec::new(),
            excalidraw: None,
            marcadas: None,
            original_primero: false,
            documentos: Vec::new(),
            tablas: HashMap::new(),
            adjuntos: Vec::new(),
            rotulo_adjuntos: String::new(),
            // El cero lo reserva el escritor de PDF para «sin imagen».
            siguiente_imagen: 1,
        }
    }

    /// Una imagen nueva, con un id que no se repite en todo lo preparado.
    /// Hace falta porque cada lienzo numera sus fotos a su manera y todos
    /// llaman `ID_PAPEL` a su papel: juntos en un PDF se pisarian.
    fn imagen(&mut self, fuente: FuenteImagen) -> u64 {
        let id = self.siguiente_imagen;
        self.siguiente_imagen += 1;
        self.imagenes.insert(id, fuente);
        id
    }

    /// Cambia los ids de las imagenes de `hoja` a los de aqui, con `fuente`
    /// diciendo de donde sale cada id de antes. Una imagen sin fuente se
    /// quita: pintaria un hueco.
    fn traducir(
        &mut self,
        hoja: &mut Hoja,
        ids: &mut HashMap<u64, u64>,
        fuente: &dyn Fn(u64) -> Option<FuenteImagen>,
    ) {
        hoja.ordenes.retain_mut(|o| {
            let Orden::Imagen { id_objeto, .. } = o else {
                return true;
            };
            let nuevo = match ids.get(id_objeto) {
                Some(n) => Some(*n),
                None => fuente(*id_objeto).map(|f| {
                    let n = self.imagen(f);
                    ids.insert(*id_objeto, n);
                    n
                }),
            };
            match nuevo {
                Some(n) => {
                    *id_objeto = n;
                    true
                }
                None => false,
            }
        });
        // Los indices de los granos apuntan a ordenes; quitar una imagen
        // los correria. No se quita nunca una tinta, pero por si acaso se
        // descartan los que ya no apuntan a una tinta.
        let ordenes = &hoja.ordenes;
        hoja.granos
            .retain(|(i, _)| matches!(ordenes.get(*i), Some(Orden::Tinta { .. })));
    }

    fn pieza(&mut self, pagina: Pagina, hoja: Hoja, fondo: ColorRgba, con_marcos: bool) {
        self.piezas.push(Pieza {
            pagina,
            hoja,
            fondo,
            con_marcos,
        });
    }

    fn marcar(&mut self, clave: &str) {
        self.marcadas
            .get_or_insert_with(BTreeSet::new)
            .insert(clave.to_string());
    }

    fn pieza_de(&self, clave: &str) -> Option<&Pieza> {
        self.piezas.iter().find(|p| p.pagina.clave == clave)
    }
}

/// El papel blanco de siempre, el del editor.
pub(crate) const BLANCO: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);
/// La tinta del texto sobre papel blanco: casi negro, como el lapiz.
const TINTA_TEXTO: ColorRgba = ColorRgba::opaco(0.106, 0.106, 0.122);
const TINTA_SUAVE: ColorRgba = ColorRgba::opaco(0.42, 0.43, 0.46);
const RAYA_TABLA: ColorRgba = ColorRgba::opaco(0.80, 0.81, 0.84);

fn a_rgba(c: pixpin_render::Color) -> ColorRgba {
    ColorRgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn pagina(
    clave: impl Into<String>,
    nombre: impl Into<String>,
    detalle: impl Into<String>,
    nivel: u8,
) -> Pagina {
    Pagina {
        clave: clave.into(),
        nombre: nombre.into(),
        detalle: detalle.into(),
        nivel,
    }
}

// ---------------------------------------------------------------------------
// Preparar

/// **Prepara lo compartible**: lee lo que haga falta del disco y lo deja en
/// hojas. Se hace en el hilo de la hoja, no en el de quien la abre: un
/// proyecto con muchos lienzos tarda en leerse y el chat no puede pararse.
pub(crate) fn preparar(cosa: Cosa, t: &Catalogo) -> Result<Preparado> {
    let mut p = preparar_sin_rotulos(cosa, t)?;
    p.rotulo_adjuntos = t.t("compartir-adjuntos");
    Ok(p)
}

fn preparar_sin_rotulos(cosa: Cosa, t: &Catalogo) -> Result<Preparado> {
    match cosa {
        Cosa::Lienzo(l) => Ok(de_lienzo_suelto(l, t)),
        Cosa::Mensajes {
            raiz,
            proyecto,
            titulo,
            mensajes,
        } => Ok(de_mensajes(&raiz, &proyecto, &titulo, &mensajes, t)),
        Cosa::Proyectos { raiz, ids } => de_proyectos(&raiz, &ids, t),
        Cosa::Hecho { titulo, extras } => {
            let mut p = Preparado::nuevo(titulo);
            p.extras = extras;
            p.original_primero = true;
            Ok(p)
        }
        Cosa::Documento(ruta) => {
            let nombre = pixpin_docs::sin_extension(&pixpin_docs::nombre(&ruta));
            let mut p = Preparado::nuevo(nombre);
            anadir_documento(&mut p, &ruta, "doc", t);
            anadir_original(&mut p, "original", "compartir-original", vec![ruta]);
            p.original_primero = true;
            Ok(p)
        }
    }
}

fn de_lienzo_suelto(l: LienzoSuelto, t: &Catalogo) -> Preparado {
    let mut p = Preparado::nuevo(if l.nombre.trim().is_empty() {
        t.t("compartir-lienzo")
    } else {
        l.nombre.clone()
    });
    let papel = l
        .papel
        .map(|(img, w, h)| (FuenteImagen::Cargada(Arc::new(img)), w, h));
    let fotos: HashMap<u64, Arc<ImagenRgba>> =
        l.fotos.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();
    let fuente = |id: u64| fotos.get(&id).cloned().map(FuenteImagen::Cargada);
    anadir_lienzo(&mut p, "lienzo", "", &l.escena, papel, &fuente, t);
    // El lienzo viaja dentro de la web y va entero como `.excalidraw`: con
    // cualquiera de los dos se sigue editando, aqui, en el movil o en
    // excalidraw.com.
    let json = excalidraw_de(&l.escena);
    p.extras.push(Extra {
        id: "excalidraw",
        clave: "compartir-editable",
        que: Entero::Escrito {
            fichero: format!("{}.excalidraw", nombre_de_fichero(&p.titulo)),
            bytes: json.clone().into_bytes(),
        },
    });
    p.excalidraw = Some(json);
    p
}

fn excalidraw_de(escena: &Escena) -> String {
    pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        escena,
    ))
}

/// Los mosaicos de un lienzo del chat, tapados (`exportar::tapado::tapar`):
/// la escena con cada uno cambiado por su imagen y esas imagenes por id.
/// `None` sin mosaicos, que es lo normal y no lee ni una imagen.
fn tapar_para_compartir(
    escena: &Escena,
    papel: Option<&(FuenteImagen, f32, f32)>,
    fotos: &dyn Fn(u64) -> Option<FuenteImagen>,
) -> Option<(Escena, HashMap<u64, Arc<ImagenRgba>>)> {
    if !escena
        .visibles()
        .any(|e| pixpin_motor2d::mosaico::tapado_de(e).is_some())
    {
        return None;
    }
    // Lo de debajo hay que pintarlo, asi que se leen el papel y las fotos.
    let mut fuentes = HashMap::new();
    if let Some((f, _, _)) = papel {
        fuentes.insert(ID_PAPEL, f.clone());
    }
    for e in escena.visibles() {
        if let pixpin_motor2d::Figura::Imagen { id_objeto } = e.figura
            && let Some(f) = fotos(id_objeto)
        {
            fuentes.insert(id_objeto, f);
        }
    }
    let lector = Lector {
        fuentes: &fuentes,
        pdfs: Default::default(),
    };
    let leidas: HashMap<u64, Arc<ImagenRgba>> = fuentes
        .keys()
        .filter_map(|id| lector.leer(*id).map(|i| (*id, i)))
        .collect();
    let de_fotos = |id: u64| leidas.get(&id).map(|i| i.as_ref());
    let lienzo = crate::ventana_editor::exportar::Lienzo {
        escena,
        seleccion: &[],
        papel: papel.and_then(|(_, w, h)| leidas.get(&ID_PAPEL).map(|i| (i.as_ref(), *w, *h))),
        fotos: &de_fotos,
        nombre: String::new(),
    };
    let t = crate::ventana_editor::exportar::tapado::tapar(&lienzo)?;
    let imagenes = t
        .imagenes
        .into_iter()
        .map(|(id, i)| (id, Arc::new(i)))
        .collect();
    Some((t.escena, imagenes))
}

/// **Un lienzo como paginas**: el lienzo entero y, si tiene marcos con algo
/// dentro, cada marco debajo, sangrado. Al abrir van marcados los marcos y
/// no el entero, que los repetiria (`alAbrir` del movil); sin marcos, el
/// entero.
fn anadir_lienzo(
    p: &mut Preparado,
    clave: &str,
    detalle: &str,
    escena: &Escena,
    papel: Option<(FuenteImagen, f32, f32)>,
    fotos: &dyn Fn(u64) -> Option<FuenteImagen>,
    t: &Catalogo,
) -> bool {
    // **Lo pixelado sale pixelado** tambien al compartir desde el chat
    // (`exportar::tapado`): el mosaico pasa a ser una imagen ya tapada. Desde
    // el editor ya llega tapado y esto no encuentra ningun mosaico.
    let tapada = tapar_para_compartir(escena, papel.as_ref(), fotos);
    let escena = tapada.as_ref().map_or(escena, |(e, _)| e);
    let tam_papel = papel.as_ref().map(|(_, w, h)| (*w, *h));
    let fuente = |id: u64| {
        if let Some(img) = tapada.as_ref().and_then(|(_, m)| m.get(&id)) {
            return Some(FuenteImagen::Cargada(img.clone()));
        }
        if id == ID_PAPEL {
            papel.as_ref().map(|(f, _, _)| f.clone())
        } else {
            fotos(id)
        }
    };
    let mut ids = HashMap::new();
    let Some(mut entero) = ex::hojas(escena, Alcance::Todo, &[], tam_papel)
        .into_iter()
        .next()
    else {
        return false;
    };
    p.traducir(&mut entero, &mut ids, &fuente);
    let hay_marcos = !pixpin_motor2d::marco::hojas_en_orden(&escena.elementos).is_empty();
    let mut marcos = if hay_marcos {
        ex::hojas(escena, Alcance::Marcos, &[], tam_papel)
    } else {
        Vec::new()
    };
    // Sin marcos con algo dentro, «cada marco» devuelve el lienzo entero
    // otra vez: no se repite.
    if marcos.len() == 1 && marcos[0].caja == entero.caja && marcos[0].nombre.is_empty() {
        marcos.clear();
    }
    let nombre_entero = if marcos.is_empty() {
        if detalle.is_empty() {
            p.titulo.clone()
        } else {
            detalle.to_string()
        }
    } else {
        t.t("compartir-lienzo-entero")
    };
    let con_marcos = !marcos.is_empty();
    p.pieza(
        pagina(clave, nombre_entero, detalle, 0),
        entero,
        BLANCO,
        con_marcos,
    );
    if !con_marcos {
        p.marcar(clave);
    }
    for (i, mut h) in marcos.into_iter().enumerate() {
        p.traducir(&mut h, &mut ids, &fuente);
        let nombre = if h.nombre.trim().is_empty() {
            format!("{} {}", t.t("compartir-marco"), i + 1)
        } else {
            h.nombre.clone()
        };
        let k = format!("{clave}|marco-{}", i + 1);
        p.pieza(
            pagina(&k, nombre, t.t("compartir-marco"), 1),
            h,
            BLANCO,
            false,
        );
        p.marcar(&k);
    }
    true
}

/// Donde esta el fichero de un mensaje en este equipo, si esta.
fn ruta_de(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<PathBuf> {
    let relativa = m.ruta.as_deref().filter(|r| !r.is_empty())?;
    pixpin_proyecto::vista::ruta_real(raiz, proyecto, relativa)
        .filter(|r| r.is_file())
        .map(|r| pixpin_proyecto::vista::con_la_extension_del_nombre(raiz, r, &m.nombre))
}

/// **Los originales** de unos mensajes: los ficheros que hay en este equipo
/// detras de ellos, en su orden. Una nota de texto no aporta nada, ni un
/// mensaje cuyo adjunto no llego a este equipo: al panel de Windows solo se
/// le pueden dar ficheros que existen.
pub(crate) fn originales(raiz: &Path, proyecto: &str, mensajes: &[Mensaje]) -> Vec<PathBuf> {
    mensajes
        .iter()
        .filter_map(|m| ruta_de(raiz, proyecto, m))
        .collect()
}

/// Si la foto del mensaje `m` tiene algo dibujado encima: en su lienzo del
/// movil o en su `.pixpin2d` de este equipo.
fn foto_con_dibujo(raiz: &Path, proyecto: &str, m: &Mensaje, foto: &Path) -> bool {
    let movil = pixpin_codec::imagen::medidas(foto).is_ok_and(|(w, h)| {
        crate::foto_anotada::dibujo_de_la_foto(raiz, proyecto, m, (w as f32, h as f32)).is_some()
    });
    movil
        || pixpin_motor2d::cargar(&dibujo_de_foto(foto)).is_ok_and(|e| e.cuantos_visibles() > 0)
}

/// **La foto de un mensaje con lo dibujado encima, en un PNG**: lo que se
/// lleva la burbuja al sacarla del chat (el usuario: «que al jalarlo no solo
/// se pase la foto sino con sus anotaciones»). Por el mismo camino que la
/// hoja de compartir, para que arrastrar y compartir den la misma imagen.
///
/// `None` si no es una foto o no tiene nada dibujado: entonces vale el
/// fichero original, que no hace falta rehacer.
pub(crate) fn foto_fusionada(
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    t: &Catalogo,
) -> Option<PathBuf> {
    if !matches!(m.clase, Some(Clase::Imagen)) {
        return None;
    }
    let foto = ruta_de(raiz, proyecto, m)?;
    if !foto_con_dibujo(raiz, proyecto, m, &foto) {
        return None;
    }
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(&foto));
    let p = de_mensajes(raiz, proyecto, &titulo, std::slice::from_ref(m), t);
    // Una carpeta por mensaje: arrastrar dos veces la misma foto pisa su
    // PNG en vez de ir llenando la temporal.
    let carpeta = carpeta_temporal().join(format!("arrastre-{}", nombre_de_fichero(&m.id)));
    match generar(&p, PNG, std::slice::from_ref(&m.id), &carpeta) {
        Ok(s) => s.ficheros.into_iter().next(),
        Err(e) => {
            tracing::warn!(?e, "no se pudo fusionar la foto con su dibujo");
            None
        }
    }
}

/// Lo dibujado sobre una foto: a su lado, con `.pixpin2d` detras (D48).
fn dibujo_de_foto(foto: &Path) -> PathBuf {
    let mut s = foto.as_os_str().to_owned();
    s.push(".pixpin2d");
    PathBuf::from(s)
}

/// El ancho al que el chat dibuja una pagina del PDF debajo de una hoja: lo
/// anotado esta en esas unidades (`fondo_lienzo::ANCHO_PAPEL_PDF`, el del movil).
const ANCHO_PAGINA: u32 = crate::fondo_lienzo::ANCHO_PAPEL_PDF as u32;

/// El papel de una pagina del PDF del proyecto, sin dibujarla todavia.
fn papel_de_pagina(raiz: &Path, proyecto: &str, pagina: u32) -> Option<(FuenteImagen, f32, f32)> {
    let pdf = crate::pdf_en_chat::documento_de(raiz, proyecto)?;
    let doc = pixpin_pdf::Documento::abrir(&pdf).ok()?;
    let (w, h) = *doc.medidas().get(pagina as usize)?;
    if w <= 0.0 {
        return None;
    }
    let ancho = ANCHO_PAGINA as f32;
    Some((
        FuenteImagen::PaginaPdf {
            pdf,
            pagina,
            ancho: ANCHO_PAGINA,
        },
        ancho,
        (ancho * h / w).round(),
    ))
}

/// El lienzo de una hoja del proyecto (`lienzos/<id>.excalidraw`) con sus
/// fotos, o `None` si no se puede leer.
fn lienzo_de_hoja(
    raiz: &Path,
    proyecto: &str,
    id: &str,
) -> Option<(Escena, HashMap<u64, PathBuf>)> {
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, id);
    let texto = std::fs::read_to_string(&ruta)
        .inspect_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "lienzo que no se pudo leer para compartir"))
        .ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto).ok()?;
    let fotos = pixpin_motor2d::excalidraw::ficheros(&lienzo)
        .into_iter()
        .filter_map(|(id, rel)| {
            Some((id, pixpin_proyecto::vista::ruta_real(raiz, proyecto, &rel)?))
        })
        .collect();
    Some((pixpin_motor2d::excalidraw::a_escena(&lienzo), fotos))
}

/// El nombre de una fila: el del mensaje o lo primero de su texto.
fn nombre_de(m: &Mensaje, t: &Catalogo) -> String {
    let nombre = if !m.nombre.trim().is_empty() {
        m.nombre.trim().to_string()
    } else if m.clase == Some(Clase::MiniApp) {
        // Una mini-app guarda su documento en el texto: el nombre es su
        // titulo, no la primera linea de un JSON.
        if m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP) {
            pixpin_proyecto::tabla::Tabla::leer(&m.texto)
                .map(|t| t.nombre)
                .unwrap_or_default()
        } else {
            pixpin_proyecto::mini::titulo(&m.texto)
        }
    } else {
        m.resumen().lines().next().unwrap_or("").trim().to_string()
    };
    let mut corto: String = nombre.chars().take(48).collect();
    if corto.len() < nombre.len() {
        corto.push('…');
    }
    if corto.is_empty() {
        t.t("compartir-sin-nombre")
    } else {
        corto
    }
}

/// **Las paginas de un mensaje**, segun lo que sea. Devuelve si puso alguna.
fn anadir_mensaje(
    p: &mut Preparado,
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    solo: bool,
    t: &Catalogo,
) -> bool {
    let clave = m.id.clone();
    let nombre = nombre_de(m, t);
    match m.clase.as_ref() {
        Some(Clase::Dibujo) | Some(Clase::Pagina) => {
            let papel = m.pagina.and_then(|n| papel_de_pagina(raiz, proyecto, n));
            let referencia = m.referencia.as_deref().filter(|r| !r.is_empty());
            let (escena, fotos) = match referencia {
                Some(id) => match lienzo_de_hoja(raiz, proyecto, id) {
                    Some(v) => v,
                    None => return false,
                },
                // Una pagina sin anotar: el papel solo.
                None if papel.is_some() => (Escena::nueva(), HashMap::new()),
                None => return false,
            };
            let fuente = |id: u64| fotos.get(&id).cloned().map(FuenteImagen::Fichero);
            let detalle = if m.clase == Some(Clase::Pagina) {
                t.t("compartir-tipo-pagina")
            } else {
                t.t("compartir-tipo-lienzo")
            };
            let puesto = anadir_lienzo(p, &clave, &detalle, &escena, papel, &fuente, t);
            if let Some(pieza) = p.piezas.iter_mut().find(|x| x.pagina.clave == clave)
                && !pieza.con_marcos
            {
                pieza.pagina.nombre = nombre;
            }
            if puesto && solo && referencia.is_some() {
                let json = excalidraw_de(&escena);
                p.extras.push(Extra {
                    id: "excalidraw",
                    clave: "compartir-editable",
                    que: Entero::Escrito {
                        fichero: format!("{}.excalidraw", nombre_de_fichero(&p.titulo)),
                        bytes: json.clone().into_bytes(),
                    },
                });
                p.excalidraw = Some(json);
            }
            puesto
        }
        Some(Clase::Imagen) => {
            let Some(foto) = ruta_de(raiz, proyecto, m) else {
                return false;
            };
            let Ok((w, h)) = pixpin_codec::imagen::medidas(&foto) else {
                return false;
            };
            // Lo dibujado en el movil vive en el lienzo propio de la foto
            // (`foto_anotada`), con la foto dentro: si tiene algo encima, se
            // comparte ese lienzo entero. Sin esto una foto anotada en el
            // movil salia limpia al compartirla o arrastrarla desde aqui.
            let del_movil = crate::foto_anotada::dibujo_de_la_foto(
                raiz,
                proyecto,
                m,
                (w as f32, h as f32),
            )
            .and_then(|_| lienzo_de_hoja(raiz, proyecto, &crate::foto_anotada::id_del_lienzo(m)));
            let puesto = if let Some((escena, fotos)) = del_movil {
                let fuente = |id: u64| fotos.get(&id).cloned().map(FuenteImagen::Fichero);
                anadir_lienzo(
                    p,
                    &clave,
                    &t.t("compartir-tipo-foto"),
                    &escena,
                    None,
                    &fuente,
                    t,
                )
            } else {
                // Con lo dibujado encima en el chat o en el editor, si lo hay.
                let escena = pixpin_motor2d::cargar(&dibujo_de_foto(&foto))
                    .unwrap_or_else(|_| Escena::nueva());
                let papel = Some((FuenteImagen::Fichero(foto), w as f32, h as f32));
                anadir_lienzo(
                    p,
                    &clave,
                    &t.t("compartir-tipo-foto"),
                    &escena,
                    papel,
                    &|_| None,
                    t,
                )
            };
            if let Some(pieza) = p.piezas.iter_mut().find(|x| x.pagina.clave == clave) {
                pieza.pagina.nombre = pixpin_docs::sin_extension(&nombre);
            }
            puesto
        }
        Some(Clase::MiniApp) => {
            if m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP)
                && let Ok(tabla) = pixpin_proyecto::tabla::Tabla::leer(&m.texto)
            {
                let titulo = if tabla.nombre.trim().is_empty() {
                    nombre.clone()
                } else {
                    tabla.nombre.clone()
                };
                let Some(hoja) = hoja_de_tabla(&tabla) else {
                    return false;
                };
                p.pieza(
                    pagina(&clave, titulo, t.t("compartir-tipo-tabla"), 0),
                    hoja,
                    BLANCO,
                    false,
                );
                p.tablas.insert(clave.clone(), tabla.clone());
                p.marcar(&clave);
                if solo {
                    p.extras.push(Extra {
                        id: "csv",
                        clave: "compartir-csv",
                        que: Entero::Escrito {
                            fichero: format!("{}.csv", nombre_de_fichero(&p.titulo)),
                            bytes: csv_de(&tabla).into_bytes(),
                        },
                    });
                }
                return true;
            }
            let texto = texto_de_miniapp(&m.texto);
            let titulo = pixpin_proyecto::mini::titulo(&m.texto);
            anadir_texto(
                p,
                &clave,
                &nombre,
                &t.t("compartir-tipo-miniapp"),
                Some(&titulo).filter(|s| !s.is_empty()).map(String::as_str),
                &texto,
                solo,
                t,
            )
        }
        Some(Clase::Archivo) => {
            let Some(ruta) = ruta_de(raiz, proyecto, m) else {
                return false;
            };
            // Solo, un PDF o un Word se abre en paginas con lo anotado; entre
            // otros mensajes va entero, en «Originales»: un PDF de trescientas
            // paginas no puede colarse en el PDF de un chat.
            solo && anadir_documento(p, &ruta, &clave, t)
        }
        Some(Clase::Voz) => {
            let texto = m.transcripcion.as_deref().unwrap_or("").trim().to_string();
            !texto.is_empty()
                && anadir_texto(
                    p,
                    &clave,
                    &nombre,
                    &t.t("compartir-tipo-voz"),
                    None,
                    &texto,
                    solo,
                    t,
                )
        }
        Some(Clase::Proyecto) => false,
        Some(Clase::Nota) | Some(Clase::Otra(_)) | None => {
            let texto = m.resumen();
            let puesta = !texto.trim().is_empty()
                && anadir_texto(
                    p,
                    &clave,
                    &nombre,
                    &t.t("compartir-tipo-nota"),
                    None,
                    &texto,
                    solo,
                    t,
                );
            // Una nota Markdown sola tambien sale como Word (`word`).
            if puesta
                && solo
                && word::es_nota(m)
                && let Some((bytes, fichero)) =
                    word::de_nota(raiz, proyecto, m, &t.t("nota-md-nueva"))
            {
                p.extras.push(Extra {
                    id: "word",
                    clave: "compartir-word",
                    que: Entero::Escrito { fichero, bytes },
                });
            }
            puesta
        }
    }
}

fn de_mensajes(
    raiz: &Path,
    proyecto: &str,
    titulo: &str,
    mensajes: &[Mensaje],
    t: &Catalogo,
) -> Preparado {
    let solo = mensajes.len() == 1;
    let titulo = match mensajes {
        [m] if matches!(
            m.clase,
            Some(Clase::Imagen) | Some(Clase::Archivo) | Some(Clase::Voz)
        ) =>
        {
            pixpin_docs::sin_extension(&nombre_de(m, t))
        }
        [m] => nombre_de(m, t),
        _ => titulo.to_string(),
    };
    let mut p = Preparado::nuevo(titulo);
    for m in mensajes {
        anadir_mensaje(&mut p, raiz, proyecto, m, solo, t);
    }
    let rutas = originales(raiz, proyecto, mensajes);
    // Las notas de voz y los ficheros, tambien dentro de la pagina web.
    p.adjuntos = adjuntos::de_originales(&rutas);
    if !rutas.is_empty() {
        let (id, clave) = if rutas.len() == 1 {
            ("original", "compartir-original")
        } else {
            ("originales", "compartir-originales")
        };
        anadir_original(&mut p, id, clave, rutas);
    }
    // Una foto o un fichero solos: lo que se suele querer es mandarlos tal
    // cual.
    p.original_primero = solo
        && mensajes.iter().any(|m| {
            matches!(
                m.clase,
                Some(Clase::Imagen) | Some(Clase::Archivo) | Some(Clase::Voz)
            )
        });
    p
}

/// **Unos proyectos de la lista**: todas sus hojas, en el orden del chat,
/// con el nombre del proyecto al lado si son varios (`CompartirPaginas.de`
/// del movil), y cada uno entero como `.pixpin` para seguir editandolo en
/// otro PixPin.
fn de_proyectos(raiz: &Path, ids: &[String], t: &Catalogo) -> Result<Preparado> {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let fichas: Vec<_> = ids.iter().filter_map(|id| indice.buscar(id)).collect();
    anyhow::ensure!(!fichas.is_empty(), "esos proyectos no estan");
    let titulo = fichas
        .iter()
        .map(|f| f.nombre.as_str())
        .collect::<Vec<_>>()
        .join(" + ");
    let mut p = Preparado::nuevo(titulo);
    let varios = fichas.len() > 1;
    let mut paquetes = Vec::new();
    for ficha in fichas {
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &ficha.id);
        let mut mensajes = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
            .unwrap_or_default()
            .mensajes;
        // Las hojas que solo se ensenan (las paginas del PDF sin dibujo)
        // tambien son del proyecto: el chat las pone igual.
        let aparato = ficha.aparato.clone().unwrap_or_default();
        mensajes.extend(pixpin_proyecto::almacen::hojas_para_ensenar(
            raiz, &ficha.id, &aparato,
        ));
        mensajes.sort_by_key(|m| m.cuando);
        p.adjuntos
            .extend(adjuntos::de_originales(&originales(raiz, &ficha.id, &mensajes)));
        let desde = p.piezas.len();
        for m in &mensajes {
            anadir_mensaje(&mut p, raiz, &ficha.id, m, false, t);
        }
        if varios {
            for x in &mut p.piezas[desde..] {
                x.pagina.detalle = if x.pagina.detalle.is_empty() {
                    ficha.nombre.clone()
                } else {
                    format!("{} · {}", ficha.nombre, x.pagina.detalle)
                };
            }
        }
        paquetes.push((
            raiz.to_path_buf(),
            ficha.id.clone(),
            format!("{}.pixpin", exportar_html::nombre_de_fichero(&ficha.nombre)),
        ));
    }
    p.extras.push(Extra {
        id: "pixpin",
        clave: "compartir-editable",
        que: Entero::Proyectos(paquetes),
    });
    Ok(p)
}

fn anadir_original(p: &mut Preparado, id: &'static str, clave: &'static str, rutas: Vec<PathBuf>) {
    let rutas: Vec<PathBuf> = rutas.into_iter().filter(|r| r.is_file()).collect();
    if rutas.is_empty() {
        return;
    }
    p.extras.push(Extra {
        id,
        clave,
        que: Entero::Ficheros(rutas),
    });
}

// ---------------------------------------------------------------------------
// Texto, tablas y mini-apps

/// La pagina de texto: un A4 a 96 ppp, con el margen del movil.
const ANCHO_A4: f32 = 794.0;
const ALTO_A4: f32 = 1123.0;
const MARGEN_A4: f32 = 64.0;
const LETRA: f32 = 14.0;
const LETRA_TITULO: f32 = 22.0;

fn texto_orden(texto: String, x: f32, y: f32, tam: f32, color: ColorRgba, ancho: f32) -> Orden {
    Orden::Texto {
        texto,
        x,
        y,
        tam,
        familia: "Segoe UI".into(),
        color,
        // Holgado: la linea ya viene partida con la tabla de anchos, y
        // DirectWrite (el PNG) mide un pelo distinto; con el ancho justo
        // volveria a partirla y se montaria sobre la siguiente.
        ancho_max: ancho * 1.5,
        negrita: false,
        cursiva: false,
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Punto2> {
    vec![
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ]
}

fn hoja_en_blanco(caja: (f32, f32, f32, f32), ordenes: Vec<Orden>) -> Hoja {
    Hoja {
        nombre: String::new(),
        caja,
        ordenes,
        marcos: Vec::new(),
        granos: Vec::new(),
        grafitos: Vec::new(),
    }
}

/// **Un texto en paginas A4**: el titulo grande arriba y el resto partido
/// en lineas con la tabla de Segoe UI (la misma que usan el SVG y el PDF),
/// pagina nueva cuando no cabe.
pub(crate) fn hojas_de_texto(titulo: Option<&str>, texto: &str) -> Vec<Hoja> {
    let ancho = ANCHO_A4 - 2.0 * MARGEN_A4;
    let mut lineas: Vec<(String, f32, ColorRgba)> = Vec::new();
    if let Some(t) = titulo {
        for l in ex::partir_texto(t, LETRA_TITULO, ancho) {
            lineas.push((l, LETRA_TITULO, TINTA_TEXTO));
        }
        lineas.push((String::new(), LETRA * 0.6, TINTA_TEXTO));
    }
    for l in ex::partir_texto(texto, LETRA, ancho) {
        lineas.push((l, LETRA, TINTA_TEXTO));
    }
    let mut hojas = Vec::new();
    let mut ordenes = Vec::new();
    let mut y = MARGEN_A4;
    for (l, tam, color) in lineas {
        let alto = tam * ex::INTERLINEA;
        if y + alto > ALTO_A4 - MARGEN_A4 && !ordenes.is_empty() {
            hojas.push(hoja_en_blanco(
                (0.0, 0.0, ANCHO_A4, ALTO_A4),
                std::mem::take(&mut ordenes),
            ));
            y = MARGEN_A4;
        }
        if !l.trim().is_empty() {
            ordenes.push(texto_orden(l, MARGEN_A4, y, tam, color, ancho));
        }
        y += alto;
    }
    if !ordenes.is_empty() || hojas.is_empty() {
        hojas.push(hoja_en_blanco((0.0, 0.0, ANCHO_A4, ALTO_A4), ordenes));
    }
    hojas
}

/// Pone un texto como pagina (o paginas, si no cabe en una). Solo, va
/// tambien entero como `.txt`.
#[allow(clippy::too_many_arguments)] // lo preparado, la fila, el titulo, el texto y si va solo
fn anadir_texto(
    p: &mut Preparado,
    clave: &str,
    nombre: &str,
    detalle: &str,
    titulo: Option<&str>,
    texto: &str,
    solo: bool,
    t: &Catalogo,
) -> bool {
    let hojas = hojas_de_texto(titulo, texto);
    for (i, h) in hojas.into_iter().enumerate() {
        let (k, n, nivel) = if i == 0 {
            (clave.to_string(), nombre.to_string(), 0)
        } else {
            (
                format!("{clave}|p{}", i + 1),
                format!("{} {}", t.t("compartir-pagina"), i + 1),
                1,
            )
        };
        p.pieza(
            pagina(&k, n, if i == 0 { detalle } else { "" }, nivel),
            h,
            BLANCO,
            false,
        );
        p.marcar(&k);
    }
    if solo {
        let entero = match titulo {
            Some(tt) => format!("{tt}\n\n{texto}"),
            None => texto.to_string(),
        };
        p.extras.push(Extra {
            id: "texto",
            clave: "compartir-texto",
            que: Entero::Escrito {
                fichero: format!("{}.txt", nombre_de_fichero(&p.titulo)),
                bytes: entero.into_bytes(),
            },
        });
    }
    true
}

/// El texto de una mini-app para leerlo: sin la linea del titulo (va
/// aparte) y con las casillas de las tareas como casillas.
fn texto_de_miniapp(documento: &str) -> String {
    pixpin_proyecto::mini::cuerpo(documento)
        .lines()
        .map(|l| {
            let s = l.trim_start();
            if let Some(r) = s
                .strip_prefix("- [x] ")
                .or_else(|| s.strip_prefix("- [X] "))
            {
                format!("☑ {r}")
            } else if let Some(r) = s.strip_prefix("- [ ] ") {
                format!("☐ {r}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

const ANCHO_COLUMNA: f32 = 96.0;
const ALTO_FILA_TABLA: f32 = 26.0;
const ANCHO_CABECERA: f32 = 40.0;
const LETRA_TABLA: f32 = 12.0;

/// El ancho de cada columna de una tabla, el puesto a mano o el de siempre.
fn anchos_de(tabla: &pixpin_proyecto::tabla::Tabla, columnas: u32) -> Vec<f32> {
    (0..columnas)
        .map(|c| {
            let letra = pixpin_proyecto::tabla::ref_a(pixpin_proyecto::tabla::Ref {
                columna: c,
                fila: 0,
            })
            .trim_end_matches(|ch: char| ch.is_ascii_digit())
            .to_string();
            tabla
                .anchos
                .get(&letra)
                .map(|w| *w as f32)
                .unwrap_or(ANCHO_COLUMNA)
                .clamp(24.0, 600.0)
        })
        .collect()
}

/// **Una tabla como hoja**: su rejilla con las letras y los numeros, y en
/// cada celda lo que vale (la formula ya calculada, como en la pantalla).
/// `None` si no tiene nada escrito.
pub(crate) fn hoja_de_tabla(tabla: &pixpin_proyecto::tabla::Tabla) -> Option<Hoja> {
    // Con el separador decimal del usuario, como en la pantalla.
    let decimal = pixpin_shell::entorno::separador_decimal();
    let (columnas, filas) = tabla.tamano();
    if columnas == 0 || filas == 0 {
        return None;
    }
    let anchos = anchos_de(tabla, columnas);
    let valores = pixpin_proyecto::formula::evaluar_todo(tabla);
    let total_x = ANCHO_CABECERA + anchos.iter().sum::<f32>();
    let total_y = ALTO_FILA_TABLA * (filas as f32 + 1.0);
    let mut ordenes = Vec::new();
    // La cabecera, en gris claro.
    let gris = ColorRgba::opaco(0.95, 0.95, 0.96);
    ordenes.push(Orden::Relleno {
        puntos: rect(0.0, 0.0, total_x, ALTO_FILA_TABLA),
        color: gris,
    });
    ordenes.push(Orden::Relleno {
        puntos: rect(0.0, 0.0, ANCHO_CABECERA, total_y),
        color: gris,
    });
    let raya = |a: (f32, f32), b: (f32, f32)| Orden::Polilinea {
        puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        color: RAYA_TABLA,
        grosor: 1.0,
        estilo: EstiloTrazo::Solido,
    };
    for f in 0..=filas + 1 {
        let y = f as f32 * ALTO_FILA_TABLA;
        ordenes.push(raya((0.0, y), (total_x, y)));
    }
    let mut x = ANCHO_CABECERA;
    ordenes.push(raya((0.0, 0.0), (0.0, total_y)));
    ordenes.push(raya((x, 0.0), (x, total_y)));
    let mut xs = Vec::with_capacity(anchos.len());
    for w in &anchos {
        xs.push(x);
        x += w;
        ordenes.push(raya((x, 0.0), (x, total_y)));
    }
    let arriba = (ALTO_FILA_TABLA - LETRA_TABLA * ex::INTERLINEA) / 2.0;
    // Las letras de las columnas y los numeros de las filas.
    for (c, x0) in xs.iter().enumerate() {
        let letra = pixpin_proyecto::tabla::ref_a(pixpin_proyecto::tabla::Ref {
            columna: c as u32,
            fila: 0,
        })
        .trim_end_matches(|ch: char| ch.is_ascii_digit())
        .to_string();
        ordenes.push(texto_orden(
            letra,
            x0 + 6.0,
            arriba,
            LETRA_TABLA,
            TINTA_SUAVE,
            anchos[c] - 8.0,
        ));
    }
    for f in 0..filas {
        let y = (f as f32 + 1.0) * ALTO_FILA_TABLA + arriba;
        ordenes.push(texto_orden(
            (f + 1).to_string(),
            6.0,
            y,
            LETRA_TABLA,
            TINTA_SUAVE,
            ANCHO_CABECERA - 8.0,
        ));
    }
    for (clave, valor) in &valores {
        let Some(r) = pixpin_proyecto::tabla::ref_de(clave) else {
            continue;
        };
        let (Some(x0), Some(w)) = (xs.get(r.columna as usize), anchos.get(r.columna as usize))
        else {
            continue;
        };
        let texto = valor.mostrar(decimal);
        // Una linea por celda, como en la pantalla: lo que no cabe se corta.
        let Some(linea) = ex::partir_texto(&texto, LETRA_TABLA, w - 8.0)
            .into_iter()
            .next()
        else {
            continue;
        };
        if linea.is_empty() {
            continue;
        }
        let y = (r.fila as f32 + 1.0) * ALTO_FILA_TABLA + arriba;
        ordenes.push(texto_orden(
            linea,
            x0 + 4.0,
            y,
            LETRA_TABLA,
            TINTA_TEXTO,
            w - 8.0,
        ));
    }
    let m = ex::MARGEN;
    Some(hoja_en_blanco((-m, -m, total_x + m, total_y + m), ordenes))
}

/// **La tabla como CSV**, con lo que vale cada celda. Con punto y coma: es
/// lo que espera el Excel de un Windows en espanol, que con comas lo mete
/// todo en una columna.
pub(crate) fn csv_de(tabla: &pixpin_proyecto::tabla::Tabla) -> String {
    // La coma decimal de la Excel en espanol va con su punto y coma.
    let decimal = pixpin_shell::entorno::separador_decimal();
    let (columnas, filas) = tabla.tamano();
    let valores = pixpin_proyecto::formula::evaluar_todo(tabla);
    let mut s = String::new();
    for f in 0..filas {
        let fila: Vec<String> = (0..columnas)
            .map(|c| {
                let clave = pixpin_proyecto::tabla::ref_a(pixpin_proyecto::tabla::Ref {
                    columna: c,
                    fila: f,
                });
                let v = valores
                    .get(&clave)
                    .map(|v| v.mostrar(decimal))
                    .unwrap_or_default();
                if v.contains([';', '"', '\n', '\r']) {
                    format!("\"{}\"", v.replace('"', "\"\""))
                } else {
                    v
                }
            })
            .collect();
        s.push_str(&fila.join(";"));
        s.push_str("\r\n");
    }
    s
}

// ---------------------------------------------------------------------------
// Documentos: PDF, Word, libros

/// Pone un documento en paginas con lo anotado. `false` si no es de los que
/// se leen aqui o no se pudo abrir.
fn anadir_documento(p: &mut Preparado, ruta: &Path, clave: &str, t: &Catalogo) -> bool {
    let nombre = pixpin_docs::nombre(ruta);
    let puesto = if crate::lector_pdf::se_abre(&nombre) {
        anadir_pdf(p, ruta, clave, t)
    } else if crate::visor::se_abre(&nombre) {
        match anadir_texto_leido(p, ruta, clave, t) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(?e, ruta = %ruta.display(), "documento que no se pudo preparar para compartir");
                false
            }
        }
    } else {
        false
    };
    if puesto {
        p.documentos.push((clave.to_string(), ruta.to_path_buf()));
    }
    puesto
}

/// Si el documento es un PDF (lo que abre el lector de PDF).
fn es_pdf(ruta: &Path) -> bool {
    crate::lector_pdf::se_abre(&pixpin_docs::nombre(ruta))
}

/// **El documento al que pertenecen todas las paginas elegidas**, si es uno
/// solo, y cuales de sus hojas son (desde 0; en un Word no cuentan: va
/// entero, que la pagina web no se corta en hojas).
fn documento_elegido<'a>(p: &'a Preparado, claves: &[String]) -> Option<(&'a Path, Vec<usize>)> {
    let (clave, ruta) = p.documentos.first()?;
    let prefijo = format!("{clave}|");
    let mut hojas = Vec::with_capacity(claves.len());
    for k in claves {
        let n: usize = k.strip_prefix(&prefijo)?.parse().ok()?;
        hojas.push(n.checked_sub(1)?);
    }
    (!hojas.is_empty()).then_some((ruta.as_path(), hojas))
}

/// **Un PDF con lo anotado**: cada hoja con su tinta
/// (`<pdf>.pixpin-anotado/hoja-N.excalidraw`) y los espacios para anotar,
/// la misma hoja que saca el «Exportar» del lector (`hoja_anotada`).
fn anadir_pdf(p: &mut Preparado, ruta: &Path, clave: &str, t: &Catalogo) -> bool {
    let doc = match pixpin_pdf::Documento::abrir(ruta) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(?e, ruta = %ruta.display(), "PDF que no se pudo abrir para compartir");
            return false;
        }
    };
    let hojas = pixpin_docs::vista::Hojas::colocar(&doc.medidas());
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let espacios = pixpin_docs::vista::espacios_del_pdf(ajustes.espacios);
    let mut anotadas = 0;
    // La de un PDF de proyecto esta en sus hojas, como la escribe el lector.
    let donde = crate::lector_pdf_proyecto::DondeVa::solo_leer(ruta);
    for i in 0..hojas.cuantas() {
        let capa = donde.para_leer(ruta, i);
        let tinta = capa.is_file().then(|| {
            pixpin_motor2d::pintado::ordenes_de_escena(
                &donde
                    .leer_capa(ruta, i, ajustes.espacios, hojas.altos[i])
                    .escena,
            )
        });
        if tinta.as_ref().is_some_and(|v| !v.is_empty()) {
            anotadas += 1;
        }
        let Some(mut h) =
            crate::lector_pdf::hoja_anotada(i, &hojas.altos, tinta.as_deref(), espacios)
        else {
            continue;
        };
        let fuente = |id: u64| {
            (id == i as u64 + 1).then(|| FuenteImagen::PaginaPdf {
                pdf: ruta.to_path_buf(),
                pagina: i as u32,
                ancho: pixpin_docs::vista::ANCHO_HOJA as u32,
            })
        };
        p.traducir(&mut h, &mut HashMap::new(), &fuente);
        let k = format!("{clave}|{}", i + 1);
        let detalle = if tinta.as_ref().is_some_and(|v| !v.is_empty()) {
            t.t("compartir-anotada")
        } else {
            String::new()
        };
        p.pieza(
            pagina(
                &k,
                format!("{} {}", t.t("compartir-pagina"), i + 1),
                detalle,
                0,
            ),
            h,
            BLANCO,
            false,
        );
        p.marcar(&k);
    }
    tracing::debug!(anotadas, "PDF preparado para compartir");
    hojas.cuantas() > 0
}

/// El ancho de la columna de un Word o un libro que nunca se anoto: el de
/// una lectura comoda, que es lo que se ve en el lector a pantalla grande.
const COLUMNA_SIN_ANOTAR: f32 = 680.0;
const MARGEN_SIN_ANOTAR: f32 = 40.0;

/// **Un Word o un libro con lo anotado**, como se ve en el lector: la misma
/// columna, la misma letra y la tinta en su sitio. La disposicion es la del
/// lector (`visor::colocar`), medida con el mismo DirectWrite fuera de un
/// fotograma, asi que lo anotado cae sobre las mismas palabras. Se parte en
/// paginas del alto de un A4 por el hueco entre dos bloques, o entre dos
/// renglones si un bloque no cabe entero.
fn anadir_texto_leido(p: &mut Preparado, ruta: &Path, clave: &str, t: &Catalogo) -> Result<bool> {
    let doc = pixpin_docs::abrir(ruta).map_err(|e| anyhow::anyhow!("{e}"))?;
    let ajustes = crate::anotado_del_adjunto::leer(ruta);
    let capa = crate::anotado_del_adjunto::leer_capa(ruta);
    let tinta = pixpin_motor2d::pintado::ordenes_de_escena(&capa.escena);
    let fijada = ajustes.letra_fijada();
    let columna = if fijada {
        ajustes.columna as f32
    } else {
        COLUMNA_SIN_ANOTAR
    };
    let dispositivo =
        pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para medir el texto")?;
    let motor = pixpin_render::MotorRender::nuevo(dispositivo.d3d())
        .context("sin motor para medir el texto")?;
    let mide = |texto: &str,
                tam: f32,
                ancho: f32,
                tramos: &[pixpin_render::Tramo],
                letra: &crate::visor::Letra| {
        motor.medir_de_lectura(texto, tam, ancho, tramos, &letra.para_pintar())
    };
    let (colocados, alto_doc) =
        crate::visor::colocar(&doc, &ajustes, columna, crate::visor::Hoja::de(ruta), &mide);
    let margen = if fijada || !tinta.is_empty() {
        pixpin_docs::vista::margen_de(columna)
    } else {
        MARGEN_SIN_ANOTAR
    };
    let fondo = a_rgba(crate::lector::FONDO);
    let (x0, x1) = (-margen, columna + margen);
    let alto_pagina = ((x1 - x0) * ALTO_A4 / ANCHO_A4).max(200.0);

    // Las ordenes del texto, en el orden de pintado.
    let mut texto: Vec<(f32, f32, Orden)> = Vec::with_capacity(colocados.len());
    let mut renglones: Vec<(f32, f32, f32)> = Vec::new(); // arriba, abajo, renglon
    for c in &colocados {
        // Una celda de tabla: su fondo y sus rayas, como en el lector.
        if let Some(k) = &c.caja {
            for (r, color) in crate::visor::ordenes_de_celda(c, k) {
                texto.push((
                    r.y,
                    r.y + r.alto,
                    Orden::Relleno {
                        puntos: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                        color: a_rgba(color),
                    },
                ));
            }
        }
        if c.texto.is_empty() && c.caja.is_some() {
            continue;
        }
        if c.texto.is_empty() {
            texto.push((
                c.y,
                c.y + 1.0,
                Orden::Relleno {
                    puntos: rect(c.sangria, c.y, c.sangria + c.ancho, c.y + 1.0),
                    color: a_rgba(c.color),
                },
            ));
            continue;
        }
        texto.push((
            c.y,
            c.y + c.alto,
            Orden::Texto {
                texto: c.texto.clone(),
                x: c.sangria,
                y: c.y,
                tam: c.tam,
                familia: c.letra.familia.into(),
                color: a_rgba(c.color),
                ancho_max: c.ancho,
                negrita: false,
                cursiva: false,
            },
        ));
        renglones.push((c.y, c.y + c.alto, c.tam * ex::INTERLINEA));
    }

    // Los cortes: donde acaba cada pagina.
    let mut cortes = Vec::new();
    let mut arriba = 0.0f32;
    let fin = alto_doc.max(renglones.last().map_or(0.0, |r| r.1));
    while arriba + alto_pagina < fin {
        let tope = arriba + alto_pagina;
        let mut corte = tope;
        if let Some((y0, _, renglon)) = renglones
            .iter()
            .find(|(y0, y1, _)| *y1 > tope && *y0 < tope)
        {
            corte = if *y0 > arriba + 1.0 {
                // El bloque que no cabe empieza en la pagina que viene.
                *y0
            } else {
                // Un bloque mas alto que una pagina: por su renglon.
                y0 + ((tope - y0) / renglon).floor().max(1.0) * renglon
            };
        }
        cortes.push(corte);
        arriba = corte;
    }
    cortes.push(fin);

    let caja_de = |o: &Orden| caja_de_orden(o);
    let mut desde = 0.0f32;
    let total = cortes.len();
    for (i, hasta) in cortes.into_iter().enumerate() {
        let mut ordenes = vec![Orden::Relleno {
            puntos: rect(x0, desde, x1, hasta),
            color: fondo,
        }];
        for (y0, y1, o) in &texto {
            if *y1 > desde && *y0 < hasta {
                ordenes.push(o.clone());
            }
        }
        for o in &tinta {
            if caja_de(o).is_none_or(|(_, a, _, b)| b > desde && a < hasta) {
                ordenes.push(o.clone());
            }
        }
        let k = format!("{clave}|{}", i + 1);
        let nombre = if total == 1 {
            pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta))
        } else {
            format!("{} {}", t.t("compartir-pagina"), i + 1)
        };
        p.pieza(
            pagina(&k, nombre, String::new(), 0),
            hoja_en_blanco((x0, desde, x1, hasta), ordenes),
            fondo,
            false,
        );
        p.marcar(&k);
        desde = hasta;
    }
    Ok(total > 0)
}

/// La caja de una orden, para saber en que pagina cae.
fn caja_de_orden(o: &Orden) -> Option<(f32, f32, f32, f32)> {
    let de = |v: &[Punto2]| {
        v.iter().fold(None, |c: Option<(f32, f32, f32, f32)>, p| {
            Some(match c {
                None => (p.x, p.y, p.x, p.y),
                Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
            })
        })
    };
    match o {
        Orden::Poligono { puntos, .. }
        | Orden::Relleno { puntos, .. }
        | Orden::Polilinea { puntos, .. } => de(puntos),
        Orden::Tinta { contorno, .. } => de(contorno),
        Orden::Velo { .. } => None,
        Orden::Texto { x, y, tam, .. } => Some((*x, *y, *x, *y + tam * ex::INTERLINEA)),
        // El numero de una cota va girado: lo que puede ocupar es el circulo
        // que barre su caja alrededor del centro.
        Orden::Rotulo { x, y, centro, .. } => {
            let r = (centro.x - x).hypot(centro.y - y);
            Some((centro.x - r, centro.y - r, centro.x + r, centro.y + r))
        }
        Orden::Imagen {
            x, y, ancho, alto, ..
        } => Some((*x, *y, x + ancho, y + alto)),
    }
}

// ---------------------------------------------------------------------------
// La hoja: formatos y paginas

/// **Lo que ensena la hoja** para lo preparado: las paginas y los formatos
/// que tocan. Con paginas: pagina web, PDF, PNG, JPG y SVG (los de una
/// pagina solo con una marcada, lo decide la hoja); detras, lo que va
/// entero. Una foto, un PDF o un Word llevan el original delante: es lo que
/// se suele querer mandar.
pub(crate) fn compartible(p: &Preparado, t: &Catalogo) -> Compartible {
    let formato = |id: &str, clave: &str, cuantas: Cuantas| Formato {
        id: id.into(),
        nombre: t.t(clave),
        cuantas,
        admite: None,
        interruptor: None,
    };
    let mut de_paginas = Vec::new();
    if !p.piezas.is_empty() {
        let mut web = formato(WEB, "compartir-web", Cuantas::Varias);
        // **La pagina web de un PDF que se lee**: con el interruptor
        // «Texto buscable» puesto, las hojas en lineas con su texto; quitado,
        // todas como imagen con el texto invisible encima (Android v0.98.1,
        // «que me de la opcion y pueda ver cual es mejor»).
        if p.documentos.iter().any(|(_, r)| es_pdf(r)) {
            web.interruptor = Some(pixpin_ui::hoja_compartir::Interruptor {
                nombre: t.t("compartir-texto-buscable"),
                detalle: t.t("compartir-texto-buscable-detalle"),
                id_apagado: WEB_IMAGEN.into(),
            });
        }
        de_paginas.push(web);
        let mut pdf = formato(PDF, "compartir-pdf", Cuantas::Varias);
        // **El PDF de un PDF que se lee** es el original con lo anotado
        // encima, y con el interruptor quitado, limpio (`formatoPdf` del
        // movil). Ver `pdf_anotado`.
        if p.documentos.iter().any(|(_, r)| es_pdf(r)) {
            pdf.interruptor = Some(pixpin_ui::hoja_compartir::Interruptor {
                nombre: t.t("compartir-con-anotaciones"),
                detalle: t.t("compartir-con-anotaciones-detalle"),
                id_apagado: PDF_LIMPIO.into(),
            });
        }
        de_paginas.push(pdf);
        de_paginas.push(formato(PNG, "compartir-png", Cuantas::Una));
        de_paginas.push(formato(JPG, "compartir-jpg", Cuantas::Una));
        de_paginas.push(formato(SVG, "compartir-svg", Cuantas::Una));
    }
    // Sin hojas pero con notas de voz o ficheros: la pagina web los lleva
    // igual, que es la forma de compartir de toda la app (8-oct-2026).
    if p.piezas.is_empty() && !p.adjuntos.is_empty() {
        de_paginas.push(formato(WEB, "compartir-web", Cuantas::Ninguna));
    }
    let enteros: Vec<Formato> = p
        .extras
        .iter()
        .map(|e| formato(e.id, e.clave, Cuantas::Ninguna))
        .collect();
    let formatos = if p.original_primero {
        enteros.into_iter().chain(de_paginas).collect()
    } else {
        de_paginas.into_iter().chain(enteros).collect()
    };
    Compartible {
        titulo: p.titulo.clone(),
        paginas: p.piezas.iter().map(|x| x.pagina.clone()).collect(),
        formatos,
        marcadas: p.marcadas.clone(),
    }
}

// ---------------------------------------------------------------------------
// Generar

/// Lo hecho: los ficheros y, si es una imagen, sus pixeles (copiar la pone
/// como imagen, no como fichero).
pub(crate) struct Salida {
    pub ficheros: Vec<PathBuf>,
    pub imagen: Option<ImagenRgba>,
    pub bytes: u64,
}

/// Lee las imagenes cuando hacen falta, abriendo cada PDF una sola vez.
struct Lector<'a> {
    fuentes: &'a HashMap<u64, FuenteImagen>,
    pdfs: std::cell::RefCell<HashMap<PathBuf, Option<pixpin_pdf::Documento>>>,
}

impl Lector<'_> {
    fn leer(&self, id: u64) -> Option<Arc<ImagenRgba>> {
        match self.fuentes.get(&id)? {
            FuenteImagen::Cargada(i) => Some(i.clone()),
            FuenteImagen::Fichero(r) => pixpin_codec::cargar(r)
                .inspect_err(|e| tracing::warn!(?e, ruta = %r.display(), "imagen que no se pudo leer para compartir"))
                .ok()
                .map(Arc::new),
            FuenteImagen::PaginaPdf { pdf, pagina, ancho } => {
                let mut pdfs = self.pdfs.borrow_mut();
                let doc = pdfs
                    .entry(pdf.clone())
                    .or_insert_with(|| pixpin_pdf::Documento::abrir(pdf).ok());
                doc.as_ref()?.renderizar(*pagina, *ancho).ok().map(Arc::new)
            }
        }
    }

    /// Las imagenes de una hoja, leidas.
    fn de_hoja(&self, hoja: &Hoja) -> HashMap<u64, Arc<ImagenRgba>> {
        let mut v = HashMap::new();
        for o in &hoja.ordenes {
            if let Orden::Imagen { id_objeto, .. } = o
                && !v.contains_key(id_objeto)
                && let Some(i) = self.leer(*id_objeto)
            {
                v.insert(*id_objeto, i);
            }
        }
        v
    }
}

/// Una imagen para meter en un SVG o una pagina web: JPEG si es opaca (una
/// foto, una pagina escaneada: pesa la quinta parte), PNG si tiene
/// transparencias, que JPEG no sabe guardar.
fn incrustar(img: &ImagenRgba) -> Option<Incrustada> {
    let opaca = img.pixeles.chunks_exact(4).all(|p| p[3] == 255);
    if opaca && img.ancho * img.alto > 64 * 64 {
        return Some(Incrustada {
            mime: "image/jpeg",
            bytes: pixpin_codec::imagen::codificar_jpg(img, 85).ok()?,
        });
    }
    Some(Incrustada {
        mime: "image/png",
        bytes: pixpin_codec::codificar_png(img).ok()?,
    })
}

fn svg_de(pieza: &Pieza, lector: &Lector<'_>, marcos: bool) -> String {
    let imagenes = lector.de_hoja(&pieza.hoja);
    exportar_svg::svg(
        &pieza.hoja,
        OpcionesSvg {
            fondo: Some(pieza.fondo),
            marcos,
        },
        &|id| imagenes.get(&id).and_then(|i| incrustar(i)),
    )
}

/// La hoja como imagen, a doble tamano (la del movil), con el `Pintor` de la
/// pantalla.
fn imagen_de(pieza: &Pieza, lector: &Lector<'_>) -> Result<ImagenRgba> {
    let imagenes = lector.de_hoja(&pieza.hoja);
    let vacia = Escena::nueva();
    let fotos = |id: u64| imagenes.get(&id).map(|i| i.as_ref());
    let lienzo = crate::ventana_editor::exportar::Lienzo {
        escena: &vacia,
        seleccion: &[],
        papel: None,
        fotos: &fotos,
        nombre: String::new(),
    };
    crate::ventana_editor::exportar::a_imagen(
        &pieza.hoja,
        escala_de(&pieza.hoja),
        Some(pieza.fondo),
        &lienzo,
    )
}

/// El lado mas largo de una imagen compartida, en pixeles: una pagina de PDF
/// a doble tamano eran 2800x3960 y seis megas para mandar por un chat.
const LADO_IMAGEN: f32 = 2400.0;

/// A que escala sale una hoja como imagen: el doble, como en el movil, sin
/// pasar de [`LADO_IMAGEN`] y nunca por debajo de su tamano (un lienzo
/// enorme no se emborrona para caber).
fn escala_de(hoja: &Hoja) -> f32 {
    (LADO_IMAGEN / hoja.ancho().max(hoja.alto()).max(1.0)).clamp(1.0, 2.0)
}

/// Un nombre de fichero para un titulo: sin lo que Windows no admite y sin
/// el punto final de una frase («lunes.» daria `lunes..txt`).
fn nombre_de_fichero(titulo: &str) -> String {
    exportar_html::nombre_de_fichero(titulo.trim_end_matches(['.', ' ']))
}

fn escribir(ruta: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(ruta, bytes).with_context(|| format!("no se pudo escribir {}", ruta.display()))
}

/// El nombre de un fichero de una sola pagina: el de todo, y el de la
/// pagina detras si hay varias.
fn nombre_de_pagina(p: &Preparado, pieza: &Pieza) -> String {
    let base = nombre_de_fichero(&p.titulo);
    if p.piezas.len() > 1 && pieza.pagina.nombre != p.titulo {
        format!("{base} - {}", nombre_de_fichero(&pieza.pagina.nombre))
    } else {
        base
    }
}

/// **Hace el fichero** del formato `formato` con las paginas `claves`, en
/// `carpeta`. Es lo que la hoja manda hacer en segundo plano para decir
/// cuanto pesa, y ese mismo fichero es el que luego se comparte: no se hace
/// dos veces.
pub(crate) fn generar(
    p: &Preparado,
    formato: &str,
    claves: &[String],
    carpeta: &Path,
) -> Result<Salida> {
    std::fs::create_dir_all(carpeta)
        .with_context(|| format!("no se pudo crear {}", carpeta.display()))?;
    let lector = Lector {
        fuentes: &p.imagenes,
        pdfs: Default::default(),
    };
    let piezas: Vec<&Pieza> = claves.iter().filter_map(|k| p.pieza_de(k)).collect();
    let base = nombre_de_fichero(&p.titulo);
    let mut imagen = None;
    let documento = if matches!(formato, WEB | WEB_IMAGEN | PDF | PDF_LIMPIO) {
        documento_elegido(p, claves)
    } else {
        None
    };
    let ficheros = match formato {
        WEB | WEB_IMAGEN if piezas.is_empty() && !p.adjuntos.is_empty() => {
            let trozo = adjuntos::html(&p.adjuntos, &p.rotulo_adjuntos);
            let ruta = carpeta.join(format!("{base}.html"));
            escribir(&ruta, adjuntos::pagina_sola(&p.titulo, &trozo).as_bytes())?;
            vec![ruta]
        }
        WEB | WEB_IMAGEN | PDF | PDF_LIMPIO if piezas.is_empty() => {
            anyhow::bail!("no hay paginas marcadas")
        }
        // Un PDF que se lee: el original con lo anotado encima en vectores
        // (o limpio), no fotos de sus hojas. Si no se deja (cifrado), como
        // antes: pintado, que mejor pesado que nada.
        PDF | PDF_LIMPIO if documento.as_ref().is_some_and(|(r, _)| es_pdf(r)) => {
            let (ruta, hojas) = documento.as_ref().context("sin documento")?;
            let bytes = match pdf_anotado::de_documento(ruta, Some(hojas), formato == PDF) {
                Ok(b) => b,
                Err(e) => {
                    tracing::info!(?e, "el PDF no se deja anotar encima; va pintado");
                    pdf::de_piezas(&piezas, &lector)?
                }
            };
            let ruta = carpeta.join(format!("{base}.pdf"));
            escribir(&ruta, &bytes)?;
            vec![ruta]
        }
        // Un documento que se lee va como documento: el texto de verdad (o
        // las hojas del PDF), lo anotado atado a su sitio y los marcadores,
        // con los mandos para seguir anotando. Ver `documento_web`.
        WEB | WEB_IMAGEN if documento.is_some() => {
            let (ruta, hojas) = documento.as_ref().context("sin documento")?;
            let pagina = if crate::lector_pdf::se_abre(&pixpin_docs::nombre(ruta)) {
                documento_web::web_de_pdf(ruta, Some(hojas), formato == WEB)?
            } else {
                documento_web::web_de_texto(ruta)?
            };
            let ruta = carpeta.join(format!("{base}.html"));
            escribir(&ruta, pagina.as_bytes())?;
            vec![ruta]
        }
        WEB | WEB_IMAGEN => {
            // Cada pagina en su clase: un dibujo como SVG y una tabla como
            // tabla que sigue calculando, en la misma pagina (J3).
            enum Web {
                Dibujo(HojaWeb),
                Tabla(exportar_html::HojaTabla),
            }
            let decimal = pixpin_shell::entorno::separador_decimal();
            let web: Vec<Web> = piezas
                .iter()
                .map(|x| match p.tablas.get(&x.pagina.clave) {
                    Some(t) => Web::Tabla(exportar_html::HojaTabla {
                        nombre: x.pagina.nombre.clone(),
                        fondo: ex::hex(x.fondo),
                        json: pixpin_proyecto::tabla_web::json(t),
                        estatica: pixpin_proyecto::tabla_web::estatica(t, 2000, 200, decimal),
                        decimal,
                    }),
                    None => Web::Dibujo(HojaWeb {
                        nombre: x.pagina.nombre.clone(),
                        svg: svg_de(x, &lector, x.con_marcos),
                        fondo: ex::hex(x.fondo),
                    }),
                })
                .collect();
            let mixtas: Vec<exportar_html::HojaDeLaPagina<'_>> = web
                .iter()
                .map(|w| match w {
                    Web::Dibujo(h) => exportar_html::HojaDeLaPagina::Dibujo(h),
                    Web::Tabla(h) => exportar_html::HojaDeLaPagina::Tabla(h),
                })
                .collect();
            let pagina = exportar_html::paginas_mixtas(
                &mixtas,
                &p.titulo,
                &base,
                exportar_html::Opciones::default(),
                p.excalidraw.as_deref(),
            )
            .context("sin hojas")?;
            let pagina =
                adjuntos::meter(pagina, &adjuntos::html(&p.adjuntos, &p.rotulo_adjuntos));
            let ruta = carpeta.join(format!("{base}.html"));
            escribir(&ruta, pagina.as_bytes())?;
            vec![ruta]
        }
        PDF => {
            let bytes = pdf::de_piezas(&piezas, &lector)?;
            let ruta = carpeta.join(format!("{base}.pdf"));
            escribir(&ruta, &bytes)?;
            vec![ruta]
        }
        PNG | JPG | SVG => {
            let pieza = *piezas.first().context("no hay pagina marcada")?;
            let nombre = nombre_de_pagina(p, pieza);
            match formato {
                SVG => {
                    let ruta = carpeta.join(format!("{nombre}.svg"));
                    escribir(&ruta, svg_de(pieza, &lector, false).as_bytes())?;
                    vec![ruta]
                }
                _ => {
                    let img = imagen_de(pieza, &lector)?;
                    let ruta = carpeta.join(format!("{nombre}.{formato}"));
                    let bytes = if formato == PNG {
                        pixpin_codec::codificar_png(&img)?
                    } else {
                        pixpin_codec::imagen::codificar_jpg(&img, 90)?
                    };
                    escribir(&ruta, &bytes)?;
                    imagen = Some(img);
                    vec![ruta]
                }
            }
        }
        otro => {
            let extra = p
                .extras
                .iter()
                .find(|e| e.id == otro)
                .with_context(|| format!("formato que no se conoce: {otro}"))?;
            match &extra.que {
                Entero::Ficheros(rutas) => rutas.clone(),
                Entero::Escrito { fichero, bytes } => {
                    let ruta = carpeta.join(fichero);
                    escribir(&ruta, bytes)?;
                    vec![ruta]
                }
                Entero::PdfDeHtml { fichero, html } => {
                    let ruta = carpeta.join(fichero);
                    crate::timeline::exportar::a_pdf(html, &ruta)
                        .map_err(|m| anyhow::anyhow!("no se pudo imprimir el PDF: {m}"))?;
                    vec![ruta]
                }
                Entero::Proyectos(paquetes) => {
                    let mut v = Vec::with_capacity(paquetes.len());
                    for (raiz, id, fichero) in paquetes {
                        let bytes = pixpin_proyecto::almacen::empaquetar(
                            raiz,
                            id,
                            pixpin_shell::entorno::ahora_utc_ms(),
                        )
                        .context("no se pudo empaquetar el proyecto")?;
                        let ruta = carpeta.join(fichero);
                        escribir(&ruta, &bytes)?;
                        v.push(ruta);
                    }
                    v
                }
            }
        }
    };
    let bytes = ficheros
        .iter()
        .filter_map(|r| std::fs::metadata(r).ok())
        .map(|m| m.len())
        .sum();
    Ok(Salida {
        ficheros,
        imagen,
        bytes,
    })
}

/// Donde se dejan los ficheros hechos para compartir: una carpeta por hoja
/// abierta dentro de la temporal. No se borran al cerrar la hoja —el panel
/// de Windows, el correo o Teams pueden leerlos despues—, sino al abrir la
/// siguiente, los que tienen mas de un dia.
pub(crate) fn carpeta_temporal() -> PathBuf {
    std::env::temp_dir().join("PixPin").join("compartir")
}

pub(crate) fn limpiar_viejos(base: &Path, edad: std::time::Duration) {
    let Ok(entradas) = std::fs::read_dir(base) else {
        return;
    };
    for e in entradas.flatten() {
        let vieja = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|d| d > edad);
        if vieja {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod pruebas;
