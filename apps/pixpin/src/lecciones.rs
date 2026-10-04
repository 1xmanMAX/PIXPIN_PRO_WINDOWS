//! **Lecciones aprendidas** en el PC (puerto de `lecciones/` de PixPin
//! Android, 3-oct-2026; ver su `docs/lecciones.md`).
//!
//! La logica (la ficha, el repaso, el buscador, las etiquetas solas, el
//! dictado repartido) esta en el crate `pixpin-lecciones`, copiada del Kotlin
//! con sus pruebas. Aqui va lo que es de la aplicacion:
//!
//! - [`almacen`]: donde viven (un archivo `.leccion` y un mensaje del chat
//!   que lo senala, como en el movil) y como se guardan y se borran con las
//!   funciones del chat.
//! - [`ficha`]: «Nueva leccion» / editar (`LeccionActivity`).
//! - [`lista`]: «Lecciones», con buscador, repaso y lista de comprobacion
//!   (`LeccionesActivity`).
//! - [`dictar`]: el microfono de las dos, con Whisper.
//!
//! Por donde se entra (como en el movil: «muy facil»): el menu de un mensaje
//! del chat («Hacer leccion»), el menu de los tres puntos del chat
//! («Lecciones», «Nueva leccion»), la bandeja, y los pedidos `leccion_nueva`
//! y `lecciones` de otros programas (`docs/protocolo-pedidos.md`).

pub mod almacen;
mod dictar;
pub mod ficha;
pub mod lista;
mod ui;

use pixpin_lecciones::buscador;
use pixpin_store::{Idioma, Ubicacion};

/// Abre la ficha de una leccion nueva. `texto` la empieza; `ficha` es el
/// chat donde ira (ninguno = «Mensajes guardados»).
pub fn nueva(
    ubicacion: Ubicacion,
    idioma: Idioma,
    aparato: &str,
    texto: Option<String>,
    de_mensaje: Option<String>,
    ficha: Option<String>,
) {
    nueva_con_fotos(ubicacion, idioma, aparato, texto, de_mensaje, ficha, Vec::new());
}

/// Como [`nueva`], con fotos que se guardan con ella (pedido
/// `leccion_nueva` con `imagenes`).
pub fn nueva_con_fotos(
    ubicacion: Ubicacion,
    idioma: Idioma,
    aparato: &str,
    texto: Option<String>,
    de_mensaje: Option<String>,
    ficha: Option<String>,
    fotos: Vec<almacen::Foto>,
) {
    ficha::abrir(ficha::Pedido {
        ubicacion,
        idioma,
        aparato: aparato.to_string(),
        que: ficha::Que::Nueva {
            texto,
            de_mensaje,
            ficha,
            dictar: false,
            fotos,
        },
    });
}

/// Abre una leccion que ya existe, por su id.
pub fn editar(ubicacion: Ubicacion, idioma: Idioma, aparato: &str, id: &str) {
    ficha::abrir(ficha::Pedido {
        ubicacion,
        idioma,
        aparato: aparato.to_string(),
        que: ficha::Que::Editar { id: id.to_string() },
    });
}

/// Abre la lista. Con `proyecto` (la ficha de un chat), las suyas primero.
pub fn lista(ubicacion: Ubicacion, idioma: Idioma, aparato: &str, proyecto: Option<String>, consulta: Option<String>) {
    lista::abrir(lista::Pedido {
        ubicacion,
        idioma,
        aparato: aparato.to_string(),
        proyecto,
        consulta,
    });
}

/// El id de la leccion de un archivo `<id>.leccion`, si lo es.
pub fn id_del_archivo(ruta: &std::path::Path) -> Option<String> {
    let nombre = ruta.file_name()?.to_str()?;
    nombre
        .strip_suffix(pixpin_lecciones::leccion::EXTENSION)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// **El aviso en el momento justo** (`AvisoDeLecciones.kt`): las lecciones
/// que tocan al entrar en el chat de un proyecto —las suyas y las que hablan
/// de lo mismo que su nombre—, la mas grave primero. Vacio si no hay.
pub fn del_proyecto(raiz: &std::path::Path, ficha: &str, nombre: &str) -> Vec<pixpin_lecciones::Leccion> {
    let todas = almacen::listar(raiz);
    let mut suyas: Vec<pixpin_lecciones::Leccion> =
        todas.iter().filter(|e| e.ficha == ficha).map(|e| e.leccion.clone()).collect();
    let fuera: Vec<buscador::Indice> = todas
        .iter()
        .filter(|e| e.ficha != ficha)
        .map(|e| buscador::Indice::nuevo(e.leccion.clone()))
        .collect();
    for l in buscador::para_el_contexto(&fuera, nombre, 5) {
        if !suyas.iter().any(|x| x.id == l.id) {
            suyas.push(l);
        }
    }
    suyas.sort_by(|a, b| b.gravedad.cmp(&a.gravedad).then(b.repeticiones.len().cmp(&a.repeticiones.len())));
    suyas
}

/// La franja del aviso: cuantas lecciones tocan y la linea de la primera
/// (lo que hara distinto, o lo aprendido).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub cuantas: usize,
    pub linea: String,
}

struct Calculado {
    cambios: u64,
    cuando: std::time::Instant,
    aviso: Option<Aviso>,
}

static AVISOS: std::sync::Mutex<Option<std::collections::HashMap<String, Calculado>>> = std::sync::Mutex::new(None);
static CALCULANDO: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// **El aviso para el chat de un proyecto, sin esperar**: lo ultimo que se
/// calculo, o nada la primera vez. Calcularlo lee los cuadernos de todos los
/// chats, asi que va en otro hilo; cuando acaba y cambia algo, el chat se
/// repinta (`ventana_chat::refrescar`). Se recalcula al guardar o borrar una
/// leccion aqui y, por lo que llegue sincronizando, cada minuto.
pub fn aviso(raiz: &std::path::Path, ficha: &str, nombre: &str) -> Option<Aviso> {
    let cambios = almacen::cambios();
    let (hay, vale) = {
        let g = AVISOS.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_ref().and_then(|m| m.get(ficha)) {
            Some(c) => (
                c.aviso.clone(),
                c.cambios == cambios && c.cuando.elapsed() < std::time::Duration::from_secs(60),
            ),
            None => (None, false),
        }
    };
    if !vale {
        let mut calculando = CALCULANDO.lock().unwrap_or_else(|e| e.into_inner());
        if !calculando.iter().any(|f| f == ficha) {
            calculando.push(ficha.to_string());
            let id = ficha.to_string();
            let (raiz, ficha, nombre) = (raiz.to_path_buf(), ficha.to_string(), nombre.to_string());
            let lanzado = std::thread::Builder::new().name("aviso-de-lecciones".into()).spawn(move || {
                let v = del_proyecto(&raiz, &ficha, &nombre);
                let aviso = v.first().map(|l| Aviso {
                    cuantas: v.len(),
                    linea: if l.proxima.trim().is_empty() { l.titulo.clone() } else { l.proxima.clone() },
                });
                let antes = {
                    let mut g = AVISOS.lock().unwrap_or_else(|e| e.into_inner());
                    let m = g.get_or_insert_with(Default::default);
                    let antes = m.get(&ficha).map(|c| c.aviso.clone());
                    m.insert(
                        ficha.clone(),
                        Calculado {
                            cambios,
                            cuando: std::time::Instant::now(),
                            aviso: aviso.clone(),
                        },
                    );
                    antes
                };
                CALCULANDO.lock().unwrap_or_else(|e| e.into_inner()).retain(|f| *f != ficha);
                if antes != Some(aviso.clone()) && (antes.is_some() || aviso.is_some()) {
                    crate::ventana_chat::refrescar();
                }
            });
            if lanzado.is_err() {
                calculando.retain(|f| *f != id);
            }
        }
    }
    hay
}
