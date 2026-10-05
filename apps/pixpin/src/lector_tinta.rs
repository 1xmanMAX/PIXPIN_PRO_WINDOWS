//! **Lo anotado encima de un documento que se lee**: la capa de tinta de
//! los lectores (Word, libro y PDF). Puerto de la capa de
//! `ui/VisorHtmlActivity.kt` (`laCapa`, v0.70-v0.85) y de `CapasDelPdf` de
//! `pdf/LectorPdfActivity.kt` (una por hoja, v0.69-v0.70).
//!
//! # Por que la tinta no tiembla
//!
//! En el movil el documento lo pinta un `WebView` y la tinta un lienzo
//! encima: dos cosas que poner de acuerdo, y la tinta iba un fotograma por
//! detras al desplazar (el fallo que la v0.85 arreglo metiendo un SVG en la
//! pagina). Aqui **no hay dos cosas**: la capa vive en las unidades del
//! documento y se pinta con el mismo `Pintor`, con la misma transformada y
//! en la misma pasada que el texto o la hoja. Desplazar o acercar cambia esa
//! transformada una vez para los dos; no hay forma de que se separen.
//!
//! # Con que se anota
//!
//! **Con las herramientas del lienzo** (2026-09-24, lo pidio el usuario: «que
//! la tinta del visor sea la misma del editor avanzado, que tenga las mismas
//! herramientas y todo»). Antes habia aqui una maquina propia con lapiz,
//! resaltador, goma y cinco colores; ahora [`Tinta`] es el anfitrion del
//! lector para el nucleo comun (`crate::dibujo`): el mismo `Gesto`, la misma
//! mano, la misma barra, el mismo panel de propiedades y las mismas teclas
//! que el lienzo, menos lo que no tiene sentido sobre un documento (ver
//! `dibujo::permitidas`) y lo que el usuario apago en los ajustes.
//!
//! # Donde se guarda (NO cambia)
//!
//! En el movil cada capa es un dibujo suelto de su almacen (`capa-doc-<huella>`
//! para un Word, `pdf-<huella>-p<n>` por hoja de PDF). Aqui va **junto al
//! documento**, como ya iban los marcadores (`.pixpin-lectura`): una carpeta
//! hermana `<nombre>.pixpin-anotado` con un `.excalidraw` por capa
//! (`capa.excalidraw` o `hoja-<n>.excalidraw`, desde 1). El formato es el
//! Excalidraw de siempre, el mismo que lee el movil: si la carpeta viaja con
//! el documento, lo anotado viaja con el. `compartir.rs` lee estas capas con
//! `Capa::leer` y `ruta_de_*`: sus firmas no cambian.
//!
//! **Salvo el PDF de un proyecto** (27-sep): ahi, como en el movil, cada
//! hoja escribe en el dibujo de su hoja del proyecto, que es lo que viaja al
//! sincronizar. Lo decide `lector_pdf_proyecto::DondeVa`.

use crate::dibujo::mano::{Atendido, Mano, Vista};
use crate::dibujo::permitidas::{self, Anfitrion};
use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{FormaCursor, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Escena};
use pixpin_render::{Color, Pintor};
use pixpin_shell::overlay::EventoOverlay;
use pixpin_sincro::anotado::MarcoDeLaHoja;
use pixpin_ui::CajaHerramientas;
use std::path::{Path, PathBuf};

/// El resaltador del lector cubre un renglon de texto de 16 px a su tamano:
/// con la tinta del movil (`tinta::ancho_del_resaltador`) un 8 pinta unos 48
/// de ancho, lo mismo que el 16 de antes con la tinta vieja (x3). Es tambien
/// lo que lo distingue de un lapiz al leer el fichero (ver
/// `con_resaltadores`): por eso se mantiene aunque el panel diga otro grosor.
const GROSOR_RESALTADOR: f32 = 8.0;
/// Lo que pesaba antes, con la tinta de grosor constante: un fichero viejo
/// lo trae y se abre al grosor de hoy para no salir el doble de gordo.
const GROSOR_RESALTADOR_VIEJO: f32 = 16.0;

/// La capa de tinta de un documento (o de una hoja de PDF).
pub struct Capa {
    pub escena: Escena,
    /// El fichero tal como se leyo: al guardar se conserva lo que el PC no
    /// entiende (lo que puso el movil), como en el editor.
    lienzo: excalidraw::Lienzo,
    /// Hay cambios sin escribir.
    pub sucia: bool,
    /// **En que unidades se leyo** (ver [`Capa::leer_en`]) y, de cada
    /// elemento, como quedo en memoria y como estaba en el fichero. Lo que no
    /// se toco vuelve tal cual del fichero: dividir y multiplicar en `f32` no
    /// siempre deja el mismo numero, y un numero cambiado es un elemento
    /// cambiado que viajaria sin que nadie lo tocara.
    corrida: (Unidades, Vec<(Elemento, Elemento)>),
}

impl Default for Capa {
    fn default() -> Self {
        Self {
            escena: Escena::nueva(),
            lienzo: excalidraw::Lienzo::vacio(),
            sucia: false,
            corrida: (Unidades::DEL_PC, Vec::new()),
        }
    }
}

/// **Las unidades del fichero de una capa respecto de las del lector**:
/// `fichero = lector * ex + dx` a lo ancho y `fichero = lector * ey + dy`
/// a lo alto. El lector del PC cuenta desde el borde de la columna de un
/// Word y desde el de la hoja de un PDF, en sus unidades; el movil no
/// siempre (ver [`Unidades::corridas`], [`Unidades::de_la_capa_del_movil`]
/// y, cuando el fichero trae su marco, [`Unidades::del_marco`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unidades {
    pub ex: f32,
    pub ey: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Cuanto pueden diferir las proporciones de las dos hojas sin avisar: un
/// uno por ciento es redondeo de medidas (la hoja del movil sale de un mapa
/// de bits); mas es que cada aparato mide otra hoja.
const PROPORCION_TOLERADA: f32 = 0.01;

impl Unidades {
    /// Las mismas: lo que solo escribe y lee el PC.
    pub const DEL_PC: Unidades = Unidades {
        ex: 1.0,
        ey: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    /// La misma escala en los dos ejes y el cero corrido solo a lo ancho: la
    /// regla de antes del marco.
    pub fn uniformes(escala: f32, dx: f32) -> Unidades {
        Unidades {
            ex: escala,
            ey: escala,
            dx,
            dy: 0.0,
        }
    }

    /// La tinta de un Word del movil cuenta desde el borde de su pagina: la
    /// columna empieza `dx` mas alla (`Maqueta.izq`).
    pub fn corridas(dx: f32) -> Unidades {
        Unidades::uniformes(1.0, dx)
    }

    /// La de una hoja de PDF del movil, con los espacios que tenga puestos
    /// (`pixpin_docs::vista::capa_del_movil`): sin ellos, dos veces y media.
    pub fn de_la_capa_del_movil(espacios: u8) -> Unidades {
        let (escala, dx) = pixpin_docs::vista::capa_del_movil(espacios);
        Unidades::uniformes(escala, dx)
    }

    /// **Las que dice el marco del fichero** (`anot-….hoja`,
    /// `pixpin_sincro::anotado::MarcoDeLaHoja`): las que llevan `propia` —la
    /// hoja en unidades del lector— a `del_fichero` —la misma hoja en las
    /// del fichero—. Si las proporciones no coinciden se encaja eje por eje:
    /// el marco manda, y cada trazo queda en el mismo sitio relativo de la
    /// hoja; el grosor y la letra, a la media (ver [`Unidades::media`]).
    /// `None` si alguno de los dos no sirve.
    pub fn del_marco(propia: &MarcoDeLaHoja, del_fichero: &MarcoDeLaHoja) -> Option<Unidades> {
        if !propia.valido() || !del_fichero.valido() {
            return None;
        }
        let (ex, ey, dx, dy) = del_fichero.desde(propia);
        let u = Unidades { ex, ey, dx, dy };
        if (ex / ey - 1.0).abs() > PROPORCION_TOLERADA {
            tracing::warn!(
                ?propia,
                ?del_fichero,
                ex,
                ey,
                "el marco de la tinta tiene otra proporcion que la hoja: se encaja eje por eje"
            );
        }
        Some(u)
    }

    /// Donde cae la hoja `propia` (unidades del lector) en estas unidades:
    /// el marco que hay que escribir junto a la tinta.
    pub fn marco_de(&self, propia: &MarcoDeLaHoja) -> MarcoDeLaHoja {
        MarcoDeLaHoja::nuevo(
            propia.x0 * self.ex + self.dx,
            propia.y0 * self.ey + self.dy,
            propia.x1 * self.ex + self.dx,
            propia.y1 * self.ey + self.dy,
        )
    }

    /// La escala de lo que no tiene eje (el grosor de un trazo, la letra):
    /// la media geometrica, que con la misma escala en los dos es esa.
    pub fn media(&self) -> f32 {
        (self.ex * self.ey).abs().sqrt()
    }

    /// Las que deshacen estas: `lector = fichero * (1/e) - d/e`.
    pub fn inversas(&self) -> Unidades {
        Unidades {
            ex: 1.0 / self.ex,
            ey: 1.0 / self.ey,
            dx: -self.dx / self.ex,
            dy: -self.dy / self.ey,
        }
    }

    /// Casi las mismas: la que sale de un marco escrito con tres decimales
    /// no es identica bit a bit a la que lo escribio.
    pub fn casi_iguales(&self, otra: &Unidades) -> bool {
        let cerca = |a: f32, b: f32, tope: f32| (a - b).abs() <= tope;
        cerca(self.ex, otra.ex, 1e-4)
            && cerca(self.ey, otra.ey, 1e-4)
            && cerca(self.dx, otra.dx, 1e-2)
            && cerca(self.dy, otra.dy, 1e-2)
    }

    fn son_las_del_pc(&self) -> bool {
        *self == Unidades::DEL_PC
    }
}

/// Lo transparente que se ve un resaltador (`HIGHLIGHTER_OPACITY` del movil).
const ALFA_RESALTADOR: f32 = pixpin_motor2d::tinta::OPACIDAD_DEL_RESALTADOR;

/// **El resaltador, tal como viaja**: Excalidraw no tiene resaltador y el
/// fichero lo guarda como un trazo mas, con su opacidad, que es lo que lo
/// hace resaltador a la vista. `excalidraw.rs` ya lo escribe al 40 % (como
/// el marcador del movil), asi que aqui no se toca: multiplicarlo otra vez
/// lo dejaba casi invisible al volver a abrirlo.
fn para_el_disco(escena: &Escena) -> Escena {
    escena.clone()
}

/// Y al leer, el trazo gordo y translucido vuelve a ser resaltador: el
/// lapiz del lector es fino y opaco, asi que no se confunden.
fn con_resaltadores(mut escena: Escena) -> Escena {
    for e in escena.elementos.iter_mut() {
        let es_resaltador =
            e.opacidad <= ALFA_RESALTADOR + 0.01 && e.grosor >= GROSOR_RESALTADOR * 0.5;
        if let (true, Figura::Lapiz { puntos, .. }) = (es_resaltador, &mut e.figura) {
            let puntos = std::mem::take(puntos);
            e.figura = Figura::Resaltador { puntos };
            e.opacidad = (e.opacidad / ALFA_RESALTADOR).min(1.0);
            if e.grosor >= GROSOR_RESALTADOR_VIEJO * 0.9 {
                e.grosor = GROSOR_RESALTADOR;
            }
        }
    }
    escena
}

/// **Corre un elemento a lo ancho**, con sus puntos: un trazo guarda sus
/// puntos donde estan (no respecto de su x), y correr la x sola lo dejaba
/// donde estaba. Asi la tinta del movil, que cuenta desde el borde de su
/// pagina, cae en el PC sobre su palabra y no `izq` mas a la derecha (K16).
/// Sin subir la version: correrlo al leer o al guardar no es cambiarlo.
fn correr(e: &mut Elemento, dx: f32, dy: f32) {
    let version = e.version;
    e.mover(dx, dy);
    e.version = version;
}

/// **Lleva un elemento a otras unidades** `u`: `q -> (q.x * ex + dx,
/// q.y * ey + dy)`, y con el su grosor y su letra, que tambien son medidas
/// del dibujo (el movil pinta el `strokeWidth` y el `fontSize` a la escala
/// de su capa: un trazo de grosor 1 sin espacios se ve alli como uno de 0,4
/// de la hoja); esos, a la escala media (`Unidades::media`). Sin subir la
/// version, como [`correr`]. Sin escala es exactamente `correr`.
fn llevar(e: &mut Elemento, u: Unidades) {
    let Unidades { ex, ey, dx, dy } = u;
    if ex == 1.0 && ey == 1.0 {
        correr(e, dx, dy);
        return;
    }
    let k = u.media();
    let version = e.version;
    let mapa = |q: &mut Punto2| {
        q.x = q.x * ex + dx;
        q.y = q.y * ey + dy;
    };
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => puntos.iter_mut().for_each(mapa),
        Figura::Region { contorno, huecos } => contorno
            .iter_mut()
            .chain(huecos.iter_mut().flatten())
            .for_each(mapa),
        Figura::Texto { tam, .. } => *tam *= k,
        _ => {}
    }
    e.x = e.x * ex + dx;
    e.y = e.y * ey + dy;
    e.ancho *= ex;
    e.alto *= ey;
    e.grosor *= k;
    if let Some(t) = e.extras.tam_letra.as_mut() {
        *t *= k;
    }
    e.version = version;
}

/// La carpeta hermana donde van las capas de un documento.
pub fn carpeta_de(documento: &Path) -> PathBuf {
    let mut nombre = documento
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    nombre.push_str(".pixpin-anotado");
    documento.with_file_name(nombre)
}

/// El fichero de la capa de un Word o un libro.
pub fn ruta_de_capa(documento: &Path) -> PathBuf {
    carpeta_de(documento).join("capa.excalidraw")
}

/// El fichero de la capa de la hoja `i` (desde 0) de un PDF. En el nombre
/// va desde 1, que es como las cuenta quien abra la carpeta.
pub fn ruta_de_hoja(documento: &Path, i: usize) -> PathBuf {
    carpeta_de(documento).join(format!("hoja-{}.excalidraw", i + 1))
}

impl Capa {
    /// Lee una capa. Si no hay fichero o no se entiende, una capa vacia:
    /// un documento sin anotar es lo normal, y uno roto no puede impedir
    /// leerlo.
    pub fn leer(ruta: &Path) -> Capa {
        let Ok(texto) = std::fs::read_to_string(ruta) else {
            return Capa::default();
        };
        match excalidraw::leer(&texto) {
            Ok(lienzo) => Capa {
                escena: con_resaltadores(excalidraw::a_escena(&lienzo)),
                lienzo,
                ..Capa::default()
            },
            Err(e) => {
                tracing::warn!(?e, ruta = %ruta.display(), "capa de tinta ilegible; se empieza vacia");
                Capa::default()
            }
        }
    }

    /// **Una capa guardada en las unidades del movil**, que cuenta desde el
    /// borde de su pagina: la columna de texto empieza `dx` mas alla (su
    /// margen izquierdo, `Maqueta.izq`). El lector del PC cuenta desde el
    /// borde de la columna, asi que al leer todo se corre `dx` a la izquierda
    /// y al guardar ([`Capa::guardar_corrida`]) se devuelve.
    #[cfg(test)]
    pub fn leer_corrida(ruta: &Path, dx: f32) -> Capa {
        Capa::leer_en(ruta, Unidades::corridas(dx))
    }

    /// **Una capa guardada en otras unidades** (ver [`Unidades`]), traida a
    /// las del lector. Se apunta como estaba cada elemento en el fichero
    /// para devolverlo tal cual si no se toca.
    pub fn leer_en(ruta: &Path, u: Unidades) -> Capa {
        let mut c = Capa::leer(ruta);
        if !u.son_las_del_pc() {
            let mut apuntes = Vec::with_capacity(c.escena.elementos.len());
            for e in c.escena.elementos.iter_mut() {
                let disco = e.clone();
                llevar(e, u.inversas());
                apuntes.push((e.clone(), disco));
            }
            c.corrida = (u, apuntes);
        }
        c
    }

    /// Escribe la capa si hay cambios, **en las unidades en que se leyo**
    /// ([`Capa::leer_en`]). Una capa que se queda vacia se escribe igual
    /// (vacia): borrar el fichero perderia lo ajeno que trajera del movil.
    pub fn guardar(&mut self, ruta: &Path) -> std::io::Result<()> {
        let u = self.corrida.0;
        self.guardar_en(ruta, u)
    }

    /// Lo mismo, devolviendo lo corrido: en el fichero la columna empieza
    /// `dx` mas alla (ver [`Capa::leer_corrida`]).
    #[cfg(test)]
    pub fn guardar_corrida(&mut self, ruta: &Path, dx: f32) -> std::io::Result<()> {
        self.guardar_en(ruta, Unidades::corridas(dx))
    }

    /// Lleva a las unidades `u` los elementos que cumplan `cuales`, como un
    /// cambio de verdad (sube su version, para que el otro aparato lo tome
    /// por nuevo), y dice cuantos. Ver
    /// `anotado_del_adjunto::lo_del_pc_a_la_capa_del_movil`.
    pub fn pasar_a(&mut self, u: Unidades, cuales: impl Fn(&Elemento) -> bool) -> usize {
        let mut n = 0;
        for e in self.escena.elementos.iter_mut().filter(|e| cuales(e)) {
            llevar(e, u);
            e.tocar();
            n += 1;
        }
        self.sucia |= n > 0;
        n
    }

    /// Escribe la capa en las unidades `u` del fichero.
    pub fn guardar_en(&mut self, ruta: &Path, u: Unidades) -> std::io::Result<()> {
        if !self.sucia {
            return Ok(());
        }
        if let Some(carpeta) = ruta.parent() {
            std::fs::create_dir_all(carpeta)?;
        }
        let mut en_disco = para_el_disco(&self.escena);
        let (antes, apuntes) = &self.corrida;
        if !u.son_las_del_pc() || !apuntes.is_empty() {
            let mismo = *antes == u;
            for e in en_disco.elementos.iter_mut() {
                // Lo que sigue como se leyo, tal cual del fichero; lo nuevo
                // o tocado, corrido.
                match apuntes.iter().find(|(m, _)| m.id == e.id) {
                    Some((memoria, disco)) if mismo && memoria == e => *e = disco.clone(),
                    _ => llevar(e, u),
                }
            }
        }
        let lienzo = excalidraw::con_escena(&self.lienzo, &en_disco);
        // Primero a un temporal y luego encima: un corte a medias no puede
        // dejar la capa buena convertida en medio fichero.
        let temporal = ruta.with_extension("excalidraw.tmp");
        std::fs::write(&temporal, excalidraw::escribir(&lienzo))?;
        std::fs::rename(&temporal, ruta)?;
        self.lienzo = lienzo;
        self.sucia = false;
        // Lo escrito es ahora lo del fichero: la proxima vez se compara con esto.
        if !u.son_las_del_pc() {
            let apuntes = self
                .escena
                .elementos
                .iter()
                .zip(en_disco.elementos.iter())
                .map(|(m, d)| (m.clone(), d.clone()))
                .collect();
            self.corrida = (u, apuntes);
        } else {
            self.corrida = (Unidades::DEL_PC, Vec::new());
        }
        Ok(())
    }

    /// Si no hay nada anotado a la vista.
    pub fn vacia(&self) -> bool {
        self.escena.visibles().next().is_none()
    }

    pub fn deshacer(&mut self) -> bool {
        let hecho = self.escena.deshacer();
        self.sucia |= hecho;
        hecho
    }

    pub fn rehacer(&mut self) -> bool {
        let hecho = self.escena.rehacer();
        self.sucia |= hecho;
        hecho
    }

    /// Todo fuera (con deshacer: es un paso mas).
    pub fn vaciar(&mut self) {
        let ids: Vec<u64> = self.escena.visibles().map(|e| e.id).collect();
        if ids.is_empty() {
            return;
        }
        self.escena.abrir_paso();
        for id in ids {
            self.escena.borrar_apuntando(id);
        }
        self.escena.cerrar_paso();
        self.sucia = true;
    }

    /// Una huella de lo que hay: cambia si se anade, se borra, se edita o
    /// se deshace algo, o si cambia el papel. Es lo que dice que hay que
    /// guardar: pasar el raton por encima con la mano no la cambia, y asi
    /// no se escribe el fichero por nada.
    fn huella(&self) -> u64 {
        let mut h = self.escena.pasos_cerrados().wrapping_mul(1_000_003);
        h = h.wrapping_add(self.escena.elementos.len() as u64);
        for e in &self.escena.elementos {
            h = h
                .wrapping_mul(31)
                .wrapping_add(e.id ^ ((e.version as u64) << 1) ^ e.borrado as u64);
        }
        let f = self.escena.fondo;
        h.wrapping_add(
            ((f.r * 255.0) as u64) << 16 | ((f.g * 255.0) as u64) << 8 | (f.b * 255.0) as u64,
        )
    }
}

/// Lo que devuelve [`Tinta::evento`] al lector.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hecho {
    /// El evento era de la tinta: el lector no hace nada mas con el.
    pub consumido: bool,
    /// Se pulso «Salir» en la barra: dejar de anotar.
    pub salir: bool,
    /// Empezo un arrastre (hay que capturar el raton).
    pub arrastra: bool,
    /// Cambio lo dibujado (para apuntar la hora del ultimo trazo).
    pub cambio: bool,
    /// El cursor que toca, si el motor dijo alguno.
    pub cursor: Option<FormaCursor>,
}

/// **Lo ya calculado de una hoja**: la geometria de sus elementos
/// (`Cache`) y lo ya subido a la GPU (`CacheTinta`, con los bitmaps del
/// grafito dentro).
///
/// Es de cada hoja y no de toda la tinta porque las dos caches van por id
/// de elemento, y cada hoja numera los suyos desde 1 (`excalidraw::a_escena`;
/// la numeracion no se puede cambiar: guardar empareja el elemento n con la
/// entrada n del fichero, que es como lo lee el movil). Con una sola para
/// todas, el elemento 1 de la hoja 1 y el de la hoja 2 se pisaban:
///
/// - `Cache` valida con la version, y dos trazos con el mismo numero de
///   puntos tienen la misma version: la hoja 2 pintaba la forma de la hoja 1
///   («algunos trazos cambian de forma»);
/// - `CacheTinta` y los bitmaps del grafito validan con la huella de la
///   forma, asi que no mezclaban, pero se echaban el uno al otro y se
///   teselaban y subian de nuevo en CADA fotograma: el lag que crecia con lo
///   anotado;
/// - y podar la cache con la escena de una hoja tiraba lo de las demas.
struct Calculado {
    geometria: Cache,
    tinta: pixpin_render::CacheTinta,
    /// El `reloj` de la tinta la ultima vez que se pinto esta hoja.
    visto: u64,
}

/// Cuantas hojas guardan lo suyo a la vez. Se ven dos o tres; el resto es
/// para ir y volver sin rehacerlo todo. Pasado el tope se suelta la que lleva
/// mas sin pintarse (volver a ella lo recalcula una vez, como la primera).
const HOJAS_CALCULADAS: usize = 8;

/// Lo calculado de `hoja`, creado la primera vez que se pinta; si ya hay
/// tantas como el tope, antes se suelta la que lleva mas sin pintarse.
fn calculado_de(
    todo: &mut std::collections::HashMap<usize, Calculado>,
    reloj: u64,
    hoja: usize,
) -> &mut Calculado {
    if !todo.contains_key(&hoja) && todo.len() >= HOJAS_CALCULADAS {
        let mas_vieja = todo.iter().min_by_key(|(_, c)| c.visto).map(|(h, _)| *h);
        if let Some(h) = mas_vieja {
            todo.remove(&h);
        }
    }
    let c = todo.entry(hoja).or_insert_with(|| Calculado {
        geometria: Cache::nueva(),
        tinta: pixpin_render::CacheTinta::nueva(),
        visto: reloj,
    });
    c.visto = reloj;
    c
}

/// **La superficie de dibujo del lector**: el anfitrion `Lector` del nucleo
/// comun. Una para todo el documento (las herramientas y el estilo son de
/// quien anota, no de cada hoja); cada evento va a la capa que diga el
/// lector (la del documento, o la de la hoja bajo el raton en un PDF).
pub struct Tinta {
    pub gesto: Gesto,
    pub mano: Mano,
    /// **Lo ya calculado, por hoja** (ver [`Calculado`]).
    calculado: std::collections::HashMap<usize, Calculado>,
    /// Cuenta cada capa pintada: lo que dice cual lleva mas sin verse.
    reloj: u64,
    /// El primero de los espacios del horno del grafito de esta tinta (uno
    /// por hoja, a partir de aqui): dos lectores en el mismo hilo tampoco
    /// se pisarian.
    espacio: u64,
    /// El lector no pega imagenes (su capa no las lleva), pero el pintado
    /// comun las pide: se le da un almacen vacio.
    imagenes: ImagenesLienzo,
    /// Donde estaba el raton, para resaltar el boton de debajo.
    raton: Option<Punto>,
    /// La capa (hoja) con la que se trabaja: la seleccion es suya. `None`
    /// hasta el primer toque.
    pub hoja: Option<usize>,
}

impl Tinta {
    pub fn nueva() -> Tinta {
        Tinta::con_mano(Mano::nueva(Anfitrion::Lector))
    }

    /// Sin leer los ajustes de tinta del usuario: las pruebas no pueden
    /// depender de como tenga puesto el suavizado.
    #[cfg(test)]
    pub fn sin_ajustes() -> Tinta {
        Tinta::con_mano(Mano::con_tinta(Anfitrion::Lector, Default::default()))
    }

    fn con_mano(mano: Mano) -> Tinta {
        let mut gesto = Gesto::nuevo();
        permitidas::asegurar(Anfitrion::Lector, &mut gesto);
        // Lejos del 0 (el lienzo) y de la hojita: cada tinta su tramo.
        static SIGUIENTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 40);
        Tinta {
            gesto,
            mano,
            calculado: std::collections::HashMap::new(),
            reloj: 0,
            espacio: SIGUIENTE.fetch_add(1 << 20, std::sync::atomic::Ordering::Relaxed),
            imagenes: ImagenesLienzo::nuevo(4096),
            raton: None,
            hoja: None,
        }
    }

    /// La barra de herramientas del lector: la del lienzo, arriba en el
    /// centro, sin lo que el lector no sabe hacer ni lo apagado.
    pub fn caja(area: Rect, escala_por_cien: u32) -> CajaHerramientas {
        CajaHerramientas::barra_superior(
            area,
            escala_por_cien,
            permitidas::botones(Anfitrion::Lector),
        )
    }

    /// Si Escape es de la tinta: cerrar el texto que se escribe, soltar lo
    /// elegido o cortar un gesto a medias. Si no, es del lector (dejar de
    /// anotar).
    pub fn quiere_escape(&self) -> bool {
        // Las hermanas abiertas de un grupo de la barra se cierran antes.
        self.mano.desplegado.is_some()
            || self.gesto.esta_escribiendo()
            || !self.gesto.seleccion.esta_vacia()
            || !self.gesto.en_reposo()
            || self.gesto.lazo.is_some()
    }

    /// Si hay un gesto a medias (no guardar en medio de un trazo).
    pub fn trazando(&self) -> bool {
        !self.gesto.en_reposo() || self.mano.borrando || self.gesto.lazo.is_some()
    }

    /// Cambia de capa (de hoja): lo elegido era de la otra.
    pub fn a_la_hoja(&mut self, hoja: usize) {
        if self.hoja != Some(hoja) {
            if self.gesto.esta_escribiendo() {
                // El texto a medias se queda en la hoja donde se escribia.
                return;
            }
            self.gesto.seleccion.limpiar();
            self.gesto.lazo = None;
            self.hoja = Some(hoja);
        }
    }

    /// **Un evento de la ventana del lector** (coordenadas del escritorio,
    /// como llegan), para la capa `capa` vista con `camara` (documento →
    /// pixeles de la ventana). `area` es la ventana en el escritorio.
    pub fn evento(
        &mut self,
        ev: &EventoOverlay,
        capa: &mut Capa,
        camara: &Camara,
        area: Rect,
        escala_por_cien: u32,
    ) -> Hecho {
        if let EventoOverlay::RatonMovido(p) = *ev {
            self.raton = Some(p);
        }
        let caja = Tinta::caja(area, escala_por_cien).con_desplegado(self.mano.desplegado);
        let vista = Vista {
            camara,
            area,
            interfaz: area,
            escala_por_cien,
        };
        let antes = capa.huella();
        let mut hecho = Hecho::default();
        let a = self.mano.interfaz(
            ev,
            &mut self.gesto,
            &mut capa.escena,
            Some(&caja),
            true,
            vista,
        );
        let a = if a.consumido || a.salir {
            a
        } else {
            let b = self.mano.herramienta(
                ev,
                &mut self.gesto,
                &mut capa.escena,
                None,
                vista,
                area.ancho as f32,
                area.alto as f32,
            );
            // Lo que no toco la caja pero si cerro el panel sigue contando.
            Atendido {
                repinte: b.repinte,
                ..b
            }
        };
        hecho.consumido = a.consumido;
        hecho.salir = a.salir;
        if let Some(paso) = &a.paso {
            hecho.cursor = Some(paso.r.cursor);
            if let Some(p) = paso.pulsado {
                hecho.arrastra = true;
                self.resaltador_de_renglon(capa, p);
            }
        }
        // La goma y el lazo tambien arrastran, sin pasar por el motor.
        if matches!(ev, EventoOverlay::BotonPulsado(_)) && a.consumido {
            hecho.arrastra = true;
        }
        if capa.huella() != antes {
            capa.sucia = true;
            hecho.cambio = true;
        }
        hecho
    }

    /// El resaltador del lector cubre un renglon (ver `GROSOR_RESALTADOR`):
    /// se le pone al nacer, que es cuando todavia no hay nada pintado de el.
    fn resaltador_de_renglon(&mut self, capa: &mut Capa, _p: Punto2) {
        if self.gesto.herramienta != Herramienta::Resaltador {
            return;
        }
        if let Some(id) = self.gesto.trazo_en_curso()
            && let Some(e) = capa.escena.buscar_mut(id)
            && matches!(e.figura, Figura::Resaltador { .. })
        {
            e.grosor = GROSOR_RESALTADOR;
        }
    }

    /// El gesto de pararse (forma rapida). `true` si hay que repintar.
    pub fn forma_rapida(&mut self, capa: &mut Capa, zoom: f32) -> bool {
        let hecho = self
            .mano
            .forma_rapida(&mut self.gesto, &mut capa.escena, zoom);
        if hecho {
            capa.sucia = true;
        }
        hecho
    }

    /// **Pinta una capa** en la misma pasada que el documento. Quien llama
    /// ya puso la transformada del documento (`Pintor::poner_vista`) y dice
    /// que se ve (`vista`, en unidades del documento), con que aumento
    /// (`zoom`, pixeles por unidad), sobre que papel (la tinta se adapta si
    /// es de noche) y si es la capa activa (la que lleva el marco de lo
    /// elegido, el lazo y compania). `hoja` dice de quien es la capa (la
    /// de un Word o un libro es la 0): cada hoja tiene lo suyo calculado.
    #[allow(clippy::too_many_arguments)]
    pub fn pintar_capa(
        &mut self,
        p: &Pintor,
        hoja: usize,
        capa: &Capa,
        vista: (f32, f32, f32, f32),
        zoom: f32,
        papel: ColorRgba,
        activa: bool,
    ) {
        self.reloj += 1;
        let calculado = calculado_de(&mut self.calculado, self.reloj, hoja);
        // Las realizaciones se teselan a una escala: lejos de ella se verian
        // facetadas (acercando) o costarian de mas (alejando). El lienzo
        // espera a que el zoom se quede quieto; el lector, que repinta
        // entero cada vez, las rehace al pasar de vez y media.
        let r = zoom / calculado.tinta.escala().max(1e-3);
        if !(0.67..=1.5).contains(&r) {
            calculado.tinta.fijar_escala(zoom);
        }
        let cache = &mut calculado.geometria;
        let cache_tinta = &mut calculado.tinta;
        let imagenes = &self.imagenes;
        let gesto = &self.gesto;
        // El grafito de cada hoja, en su espacio del horno: por lo mismo que
        // las caches van por hoja (sus ids se repiten de una a otra).
        let espacio = self.espacio + hoja as u64;
        pixpin_motor2d::tinta::grafito::en_espacio(espacio, || {
            crate::dibujo::tema::con_papel(Some(papel), || {
                crate::dibujo::pintar::pintar_escena(
                    p,
                    &capa.escena,
                    cache,
                    cache_tinta,
                    imagenes,
                    vista,
                    zoom,
                    |id| activa && gesto.elemento_en_curso().is_some_and(|(en, _)| en == id),
                );
                if activa {
                    crate::dibujo::pintar::pintar_encima(
                        p,
                        gesto,
                        &capa.escena,
                        vista,
                        zoom,
                        imagenes,
                        false,
                        false,
                    );
                }
            })
        });
        // Lo borrado y compactado no se queda en la cache para siempre. Con
        // la escena de SU hoja: la de otra le tiraria todo.
        if cache.sobran(capa.escena.cuantos_visibles()) {
            cache.podar(&capa.escena);
        }
    }

    /// Cuantas veces se ha teselado algo de nuevo, en todas las hojas: lo
    /// que mide que anotar no rehaga en cada fotograma lo que no cambio.
    #[cfg(test)]
    pub fn realizaciones(&self) -> u64 {
        self.calculado.values().map(|c| c.tinta.realizadas()).sum()
    }

    /// Cuantos bitmaps de grafito se han subido a la GPU, en todas las hojas.
    #[cfg(test)]
    pub fn subidas_de_grafito(&self) -> u64 {
        self.calculado
            .values()
            .map(|c| c.tinta.grano.grafito.subidas())
            .sum()
    }

    /// **Suelta lo calculado de todas las hojas.** Las caches validan con la
    /// version, y una capa vuelta a leer en otras unidades (el lector de PDF
    /// al poner o quitar un espacio en un adjunto, o al pasar a proyecto)
    /// tiene los mismos ids y versiones con otra geometria: sin esto se
    /// pintaria la de antes.
    pub fn olvidar_lo_calculado(&mut self) {
        self.calculado.clear();
    }

    /// La cache de geometria de `hoja`, la misma que usa `pintar_capa`.
    #[cfg(test)]
    fn geometria_de(&mut self, hoja: usize) -> &mut Cache {
        self.reloj += 1;
        &mut calculado_de(&mut self.calculado, self.reloj, hoja).geometria
    }

    /// La barra y el panel de propiedades, en pixeles de la ventana (quien
    /// llama ya quito la transformada del documento). `capa` es la activa:
    /// el panel ensena lo que hay elegido en ella.
    pub fn pintar_interfaz(
        &self,
        p: &Pintor,
        capa: Option<&Capa>,
        area: Rect,
        escala_por_cien: u32,
    ) {
        let caja = Tinta::caja(area, escala_por_cien).con_desplegado(self.mano.desplegado);
        // La caja y el panel viven en coordenadas del escritorio (son las de
        // los clics); se pintan en las de la ventana.
        p.desplazar(-(area.x as f32), -(area.y as f32));
        crate::caja_dibujo::pintar_barra(
            p,
            &caja,
            self.gesto.herramienta,
            escala_por_cien,
            self.raton,
            |b| match b {
                pixpin_ui::BotonCaja::Elegir(h) => crate::dibujo::teclas::tecla_de(h)
                    .filter(|_| permitidas::permitida(Anfitrion::Lector, h)),
                _ => None,
            },
        );
        let vacia = Escena::nueva();
        let escena = capa.map_or(&vacia, |c| &c.escena);
        if let Some(panel) =
            crate::panel_dibujo::panel_para(&self.gesto, escena, area, escala_por_cien)
        {
            crate::panel_dibujo::pintar(p, &panel, escala_por_cien);
        }
        p.desplazar(0.0, 0.0);
    }

    /// Traza con la herramienta de ahora por `puntos` (unidades del
    /// documento), como lo haria el raton: pulsar, mover y soltar. Para las
    /// pruebas y las fotos de muestra, sin ventana.
    #[cfg(test)]
    pub fn trazar(&mut self, capa: &mut Capa, puntos: &[Punto2]) {
        let Some(ultimo) = puntos.last() else {
            return;
        };
        self.empezar_trazo(capa, puntos);
        self.acabar_trazo(capa, *ultimo);
    }

    /// Pulsar y mover por `puntos` sin soltar: el trazo queda en curso,
    /// como a mitad del arrastre. Para las pruebas que pintan un trazo a
    /// medias y luego suelto.
    #[cfg(test)]
    pub fn empezar_trazo(&mut self, capa: &mut Capa, puntos: &[Punto2]) {
        use pixpin_motor2d::gesto::EventoGesto;
        let camara = Camara::nueva();
        let Some((primero, resto)) = puntos.split_first() else {
            return;
        };
        let paso = self.mano.al_motor(
            EventoGesto::Pulsar {
                p: *primero,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut self.gesto,
            &mut capa.escena,
            &camara,
        );
        if let Some(p) = paso.pulsado {
            self.resaltador_de_renglon(capa, p);
        }
        for q in resto {
            self.seguir_trazo(capa, *q);
        }
    }

    /// Un punto mas del trazo en curso, como un movimiento del raton.
    #[cfg(test)]
    pub fn seguir_trazo(&mut self, capa: &mut Capa, q: Punto2) {
        use pixpin_motor2d::gesto::EventoGesto;
        self.mano.al_motor(
            EventoGesto::Mover {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut self.gesto,
            &mut capa.escena,
            &Camara::nueva(),
        );
    }

    /// Suelta el trazo en curso en `ultimo`.
    #[cfg(test)]
    pub fn acabar_trazo(&mut self, capa: &mut Capa, ultimo: Punto2) {
        use pixpin_motor2d::gesto::EventoGesto;
        let camara = Camara::nueva();
        let antes = capa.huella();
        self.mano.al_motor(
            EventoGesto::Soltar { p: ultimo },
            &mut self.gesto,
            &mut capa.escena,
            &camara,
        );
        self.mano.quieto = None;
        if capa.huella() != antes {
            capa.sucia = true;
        }
    }
}

/// Si la tecla `vk` (una letra) elige ahora una herramienta de la tinta del
/// lector. Anotando, esa letra es de la herramienta y no del lector (la M de
/// la mano no pone un marcador ni la G de la escala grafica abre los
/// ajustes); una apagada no, y entonces la letra vuelve a ser del lector.
pub fn letra_de_herramienta(vk: u32) -> bool {
    (0x41..=0x5A).contains(&vk)
        && permitidas::herramienta_de_letra(Anfitrion::Lector, vk as u8 as char).is_some()
}

/// El papel de un lector como color del motor: contra el se adapta la tinta.
pub fn papel(c: Color) -> ColorRgba {
    ColorRgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: 1.0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tinta_de_prueba(h: Herramienta) -> Tinta {
        let mut t = Tinta::con_mano(Mano::con_tinta(Anfitrion::Lector, Default::default()));
        crate::dibujo::teclas::elegir_herramienta(&mut t.gesto, h);
        t
    }

    fn capa_con_un_trazo() -> Capa {
        let mut c = Capa::default();
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        t.trazar(
            &mut c,
            &[
                Punto2::nuevo(10.0, 10.0),
                Punto2::nuevo(30.0, 12.0),
                Punto2::nuevo(60.0, 20.0),
            ],
        );
        c
    }

    #[test]
    fn un_trazo_queda_en_unidades_del_documento_y_ensucia_la_capa() {
        let c = capa_con_un_trazo();
        let e = c.escena.visibles().next().expect("hay trazo");
        assert!(matches!(e.figura, Figura::Lapiz { .. }));
        let (x0, _, x1, _) = e.caja();
        assert!(x0 <= 10.5 && x1 >= 59.5, "{x0} {x1}");
        assert!(c.sucia);
    }

    /// K16: la tinta del movil cuenta desde el borde de su pagina y el lector
    /// desde el de la columna. Correrla es correr sus PUNTOS, no solo su x:
    /// antes el trazo del movil salia `izq` pixeles a la derecha de su
    /// palabra, y el del PC viajaba sin correr.
    #[test]
    fn la_tinta_corrida_mueve_sus_puntos_y_lo_que_no_se_toca_vuelve_igual_al_fichero() {
        let dir =
            std::env::temp_dir().join(format!("pixpin-corrida-puntos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("anot.excalidraw");
        let fichero = r#"{"type":"excalidraw","elements":[{"id":"m1","type":"freedraw","x":364.1875,"y":100.0,"width":20,"height":4,"seed":1,"version":3,"versionNonce":1,"updated":1,"points":[[0,0],[20.5,4]]}],"files":{}}"#;
        std::fs::write(&ruta, fichero).unwrap();
        let mut c = Capa::leer_corrida(&ruta, 256.0);
        let e = c.escena.visibles().next().unwrap().clone();
        let sin = Capa::leer(&ruta).escena.visibles().next().unwrap().caja();
        let (x0, _, x1, _) = e.caja();
        assert!(
            (sin.0 - x0 - 256.0).abs() < 1e-3 && (sin.2 - x1 - 256.0).abs() < 1e-3,
            "la caja entera, corrida: {sin:?} {x0} {x1}"
        );
        assert_eq!(e.version, 3, "correr al leer no es cambiar");
        // Un trazo nuevo del PC en la columna (x 10..30) va al fichero corrido.
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        t.trazar(
            &mut c,
            &[Punto2::nuevo(10.0, 50.0), Punto2::nuevo(30.0, 50.0)],
        );
        c.guardar_corrida(&ruta, 256.0).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
        let els = v["elements"].as_array().unwrap();
        assert_eq!(
            els[0]["x"].as_f64(),
            Some(364.1875),
            "el del movil, tal cual"
        );
        assert_eq!(els[0]["version"].as_u64(), Some(3));
        let nuevo = &els[1];
        let x = nuevo["x"].as_f64().unwrap();
        let p0 = nuevo["points"][0][0]
            .as_f64()
            .or_else(|| nuevo["points"][0]["x"].as_f64())
            .unwrap();
        assert!(
            (x + p0 - 266.0).abs() < 0.5,
            "el del PC, en unidades de la pagina: {x} + {p0}"
        );
        // Y al volver a leerlo cae donde se dibujo.
        let otra = Capa::leer_corrida(&ruta, 256.0);
        let n = otra.escena.visibles().nth(1).unwrap();
        let hecho = c.escena.visibles().nth(1).unwrap().caja();
        assert!(
            (n.caja().0 - hecho.0).abs() < 0.01,
            "{:?} {hecho:?}",
            n.caja()
        );
        // Caso negativo: sin corrida (un documento suelto) nada se mueve.
        let quieta = Capa::leer_corrida(&ruta, 0.0);
        assert!((quieta.escena.visibles().next().unwrap().caja().0 - sin.0).abs() < 1e-3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Una hoja de PDF tal como la manda el movil** (forma exacta de
    /// `anot-<uid>-p<n>`, v0.97: puntos `{x,y}` relativos a su x/y, presiones,
    /// `strokeWidth` 1, un texto con su `fontSize`, y la vista de la capa sin
    /// espacios). Los numeros son los de `anot-VRQH2DA7AB-p1` del usuario: la
    /// «upc» escrita encima del logo, a 139 de letra en unidades del movil.
    const HOJA_DEL_MOVIL: &str = r##"{"alfileres":[],"backgroundColor":"#ffffff","elements":[{"angle":0.0,"backgroundColor":"transparent","fillStyle":"solid","groupIds":[],"height":151.4812893337671,"id":"jDNyxAFkaxm2qZ-KsluKG","isDeleted":false,"locked":false,"material":"lisa","opacity":100,"points":[{"x":0.0,"y":0.0},{"x":-3.574230052806797,"y":-8.85823567708303},{"x":17.0,"y":142.6}],"presionFirme":false,"pressures":[0.5,0.6,0.7],"roughness":0,"scale":[1.0,1.0],"seed":656548593,"simulatePressure":true,"strokeColor":"#f08c00","strokeStyle":"solid","strokeWidth":1.0,"type":"freedraw","updated":1790717124003,"version":1,"versionNonce":-1460576999,"width":20.561416060836336,"x":844.1965456362125,"y":3080.10412145544},{"angle":0.0,"backgroundColor":"transparent","fillStyle":"solid","fontFamily":5,"fontSize":138.863841869213,"groupIds":[],"height":173.57980233651622,"id":"feE_wn2xlXlpKLQ278O5D","isDeleted":false,"locked":false,"opacity":100,"roughness":0,"seed":716826725,"strokeColor":"#f08c00","strokeStyle":"solid","strokeWidth":2.0,"text":"upc","textAlign":"left","type":"text","updated":1790717160057,"version":6,"versionNonce":1013548572,"verticalAlign":"top","width":231.41507749204288,"x":648.0419300220633,"y":167.14183666087965}],"files":{},"viewport":{"scrollX":1050.0,"scrollY":0.0,"zoom":0.30857142857142855},"vista":"cero"}"##;

    fn hoja_del_movil_en(etiqueta: &str) -> (PathBuf, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("pixpin-unidades-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("anot-VRQH2DA7AB-p1.excalidraw");
        std::fs::write(&ruta, HOJA_DEL_MOVIL).unwrap();
        (dir, ruta)
    }

    fn elementos(ruta: &Path) -> Vec<serde_json::Value> {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(ruta).unwrap()).unwrap();
        v["elements"].as_array().unwrap().clone()
    }

    /// Lo que el usuario vio: «lo mando como 1 y aqui llega como 10». Sin
    /// espacios en el movil, cada unidad de la hoja son 2,5 del fichero y la
    /// hoja empieza en -1050: leida tal cual, la letra salia dos veces y media
    /// mas grande (y mas gorda) y fuera de su sitio.
    #[test]
    fn la_tinta_del_movil_sin_espacios_se_lee_al_tamano_de_la_hoja() {
        let (dir, ruta) = hoja_del_movil_en("leer");
        let tal_cual = Capa::leer(&ruta);
        let c = Capa::leer_en(&ruta, Unidades::de_la_capa_del_movil(0));
        let trazo = |c: &Capa| {
            c.escena
                .visibles()
                .find(|e| matches!(e.figura, Figura::Lapiz { .. }))
                .unwrap()
                .clone()
        };
        let (a, b) = (trazo(&tal_cual), trazo(&c));
        let (ca, cb) = (a.caja(), b.caja());
        assert!(
            ((ca.0 + 1050.0) / 2.5 - cb.0).abs() < 0.01,
            "a lo ancho: {ca:?} {cb:?}"
        );
        assert!((ca.1 / 2.5 - cb.1).abs() < 0.01, "a lo alto: {ca:?} {cb:?}");
        let alto = |c: (f32, f32, f32, f32)| c.3 - c.1;
        assert!(
            (alto(ca) / alto(cb) - 2.5).abs() < 0.01,
            "el trazo, a su tamano: {} {}",
            alto(ca),
            alto(cb)
        );
        assert!(
            (b.grosor * 2.5 - a.grosor).abs() < 1e-4,
            "y a su grosor: {} {}",
            a.grosor,
            b.grosor
        );
        assert_eq!(b.version, a.version, "leer en otras unidades no es cambiar");
        // La letra, igual: 139 del movil son 55,5 de la hoja.
        let letra = |c: &Capa| {
            c.escena
                .visibles()
                .find_map(|e| match &e.figura {
                    Figura::Texto { tam, .. } => Some(*tam),
                    _ => None,
                })
                .unwrap()
        };
        assert!((letra(&c) - 138.863_84 / 2.5).abs() < 0.01, "{}", letra(&c));
        // Y cae dentro de la hoja (1400 de ancho, 1980 de alto), no en x 844.
        assert!(cb.0 > 700.0 && cb.2 < 800.0 && cb.3 < 1300.0, "{cb:?}");
        // Caso negativo: con los dos espacios el movil ya guarda en la hoja.
        let igual = Capa::leer_en(&ruta, Unidades::de_la_capa_del_movil(3));
        assert_eq!(trazo(&igual).caja(), ca);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ida y vuelta: lo del movil que no se toca vuelve al fichero identico
    /// (ni un decimal cambiado, que viajaria), y lo que se dibuja en el PC
    /// va en las unidades del movil y al volver a leerlo queda igual, por
    /// muchas veces que se guarde.
    #[test]
    fn la_ida_y_vuelta_con_el_movil_no_cambia_el_tamano_de_nada() {
        let (dir, ruta) = hoja_del_movil_en("vuelta");
        let u = Unidades::de_la_capa_del_movil(0);
        let antes = elementos(&ruta);
        let mut c = Capa::leer_en(&ruta, u);
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        t.trazar(
            &mut c,
            &[
                Punto2::nuevo(100.0, 500.0),
                Punto2::nuevo(200.0, 520.0),
                Punto2::nuevo(300.0, 500.0),
            ],
        );
        let dibujado = c.escena.visibles().last().unwrap().clone();
        c.guardar(&ruta).unwrap();
        let despues = elementos(&ruta);
        for (a, d) in antes.iter().zip(despues.iter()) {
            for k in [
                "x",
                "y",
                "width",
                "height",
                "strokeWidth",
                "fontSize",
                "version",
            ] {
                assert_eq!(a[k], d[k], "{k} del movil, tal cual");
            }
            assert_eq!(a["points"], d["points"], "los puntos del movil, tal cual");
        }
        // Lo del PC, en las unidades del movil: 100 de la hoja son -800.
        let nuevo = &despues[2];
        let x = nuevo["x"].as_f64().unwrap();
        assert!((x - (100.0 * 2.5 - 1050.0)).abs() < 3.0, "{x}");
        assert!(
            (nuevo["strokeWidth"].as_f64().unwrap() - f64::from(dibujado.grosor) * 2.5).abs()
                < 1e-3
        );
        // Y vuelve igual, guardando una y otra vez.
        for _ in 0..4 {
            let mut otra = Capa::leer_en(&ruta, u);
            let e = otra.escena.visibles().last().unwrap().clone();
            let (a, b) = (e.caja(), dibujado.caja());
            assert!(
                (a.0 - b.0).abs() < 0.01 && (a.3 - b.3).abs() < 0.01,
                "{a:?} {b:?}"
            );
            assert!((e.grosor - dibujado.grosor).abs() < 1e-4);
            otra.sucia = true;
            otra.guardar(&ruta).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn deshacer_quita_el_trazo_entero_y_rehacer_lo_devuelve() {
        let mut c = capa_con_un_trazo();
        assert!(c.deshacer());
        assert!(c.vacia(), "el trazo es un solo paso, no uno por punto");
        assert!(c.rehacer());
        assert!(!c.vacia());
        let mut vacia = Capa::default();
        assert!(!vacia.deshacer(), "sin nada no hay que deshacer");
    }

    #[test]
    fn en_el_lector_se_dibujan_tambien_formas_flechas_y_texto_del_lienzo() {
        let mut c = Capa::default();
        for h in [
            Herramienta::Rectangulo,
            Herramienta::Flecha,
            Herramienta::Elipse,
        ] {
            let mut t = tinta_de_prueba(h);
            t.trazar(
                &mut c,
                &[Punto2::nuevo(100.0, 100.0), Punto2::nuevo(180.0, 160.0)],
            );
        }
        let figuras: Vec<&Figura> = c.escena.visibles().map(|e| &e.figura).collect();
        assert!(matches!(figuras[0], Figura::Rectangulo));
        assert!(matches!(figuras[1], Figura::Flecha { .. }));
        assert!(matches!(figuras[2], Figura::Elipse));
    }

    #[test]
    fn la_goma_del_lienzo_se_lleva_el_trazo_que_toca_y_no_el_de_al_lado() {
        let mut c = capa_con_un_trazo();
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        t.trazar(
            &mut c,
            &[Punto2::nuevo(500.0, 500.0), Punto2::nuevo(520.0, 500.0)],
        );
        assert_eq!(c.escena.cuantos_visibles(), 2);
        // La goma va por la mano (no es del motor), con eventos de ventana.
        let mut goma = tinta_de_prueba(Herramienta::Borrador);
        let camara = Camara::nueva();
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1600,
            alto: 900,
        };
        // Lejos de la barra, que esta arriba.
        let en = |x: i32, y: i32| Punto { x, y };
        goma.evento(
            &EventoOverlay::BotonPulsado(en(30, 12 + 400)),
            &mut c,
            &Camara {
                y: -400.0,
                ..camara
            },
            area,
            100,
        );
        goma.evento(
            &EventoOverlay::BotonSoltado(en(30, 412)),
            &mut c,
            &Camara {
                y: -400.0,
                ..camara
            },
            area,
            100,
        );
        assert_eq!(c.escena.cuantos_visibles(), 1);
        // Caso negativo: lejos de todo no borra nada.
        goma.evento(
            &EventoOverlay::BotonPulsado(en(1200, 800)),
            &mut c,
            &camara,
            area,
            100,
        );
        goma.evento(
            &EventoOverlay::BotonSoltado(en(1200, 800)),
            &mut c,
            &camara,
            area,
            100,
        );
        assert_eq!(c.escena.cuantos_visibles(), 1);
        c.deshacer();
        assert_eq!(c.escena.cuantos_visibles(), 2);
    }

    #[test]
    fn una_herramienta_apagada_no_sale_en_la_barra_del_lector_ni_responde_a_su_letra() {
        permitidas::fijar(pixpin_store::herramientas::Herramientas {
            apagadas: vec!["rectangulo".into()],
            ..Default::default()
        });
        let caja = Tinta::caja(
            Rect {
                x: 0,
                y: 0,
                ancho: 1600,
                alto: 900,
            },
            100,
        );
        assert!(
            !caja
                .miembros(pixpin_ui::GrupoBarra::Formas)
                .contains(&pixpin_ui::BotonCaja::Elegir(Herramienta::Rectangulo))
        );
        // Caso negativo: el resto de las formas sigue en su grupo.
        assert!(
            caja.miembros(pixpin_ui::GrupoBarra::Formas)
                .contains(&pixpin_ui::BotonCaja::Elegir(Herramienta::Elipse))
        );
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        let mut c = Capa::default();
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1600,
            alto: 900,
        };
        // La «r» elige el resaltador (su letra) y no hay letra que llegue al
        // rectangulo; pero un atajo que lo eligiera tampoco valdria.
        t.evento(
            &EventoOverlay::Caracter('r'),
            &mut c,
            &Camara::nueva(),
            area,
            100,
        );
        assert_eq!(t.gesto.herramienta, Herramienta::Resaltador);
        // Caso negativo con una que si tiene letra: apagada, su letra no hace nada.
        permitidas::fijar(pixpin_store::herramientas::Herramientas {
            apagadas: vec!["texto".into()],
            ..Default::default()
        });
        let h = t.evento(
            &EventoOverlay::Caracter('t'),
            &mut c,
            &Camara::nueva(),
            area,
            100,
        );
        assert_ne!(t.gesto.herramienta, Herramienta::Texto);
        assert!(!h.cambio);
        permitidas::fijar(Default::default());
    }

    #[test]
    fn el_resaltador_se_guarda_al_35_por_ciento_y_la_version_de_antes_lo_lee_igual() {
        let dir =
            std::env::temp_dir().join(format!("pixpin-lector-resaltador-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = ruta_de_capa(&dir.join("libro.epub"));
        let mut c = capa_con_un_trazo();
        let mut t = tinta_de_prueba(Herramienta::Resaltador);
        t.trazar(
            &mut c,
            &[Punto2::nuevo(0.0, 50.0), Punto2::nuevo(120.0, 50.0)],
        );
        let r = c.escena.visibles().nth(1).unwrap();
        assert_eq!(r.grosor, GROSOR_RESALTADOR, "cubre un renglon, como antes");
        c.guardar(&ruta).unwrap();
        // En el fichero, con la opacidad con la que se ve: asi lo ven el
        // movil y excalidraw.com.
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(
            texto.contains("\"opacity\": 40") || texto.contains("\"opacity\":40"),
            "{texto}"
        );
        let leida = Capa::leer(&ruta);
        let figuras: Vec<bool> = leida
            .escena
            .visibles()
            .map(|e| matches!(e.figura, Figura::Resaltador { .. }))
            .collect();
        // Caso negativo: el lapiz sigue siendo lapiz.
        assert_eq!(figuras, [false, true]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_anotado_se_guarda_en_el_mismo_fichero_de_siempre_y_se_lee_igual() {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-lector-tinta-guardar-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("apuntes.docx");
        let ruta = ruta_de_capa(&doc);
        assert!(ruta.starts_with(dir.join("apuntes.docx.pixpin-anotado")));
        assert!(ruta.ends_with("capa.excalidraw"));
        let mut c = capa_con_un_trazo();
        let mut t = tinta_de_prueba(Herramienta::Flecha);
        t.trazar(
            &mut c,
            &[Punto2::nuevo(0.0, 0.0), Punto2::nuevo(80.0, 40.0)],
        );
        c.guardar(&ruta).unwrap();
        assert!(!c.sucia);
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(
            texto.contains("\"type\": \"excalidraw\"") || texto.contains("\"type\":\"excalidraw\"")
        );
        assert!(
            texto.contains("\"arrow\""),
            "la flecha viaja como la del lienzo"
        );
        let leida = Capa::leer(&ruta);
        assert_eq!(leida.escena.cuantos_visibles(), 2);
        assert!(
            !ruta.with_extension("excalidraw.tmp").exists(),
            "no queda el temporal"
        );
        // Guardar sin cambios no escribe.
        let mut otra = Capa::leer(&ruta);
        std::fs::remove_file(&ruta).unwrap();
        otra.guardar(&ruta).unwrap();
        assert!(!ruta.exists(), "sin cambios no se toca el disco");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pasar_el_raton_con_la_mano_no_ensucia_la_capa() {
        let mut c = capa_con_un_trazo();
        c.sucia = false;
        let mut t = tinta_de_prueba(Herramienta::Mano);
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1600,
            alto: 900,
        };
        let h = t.evento(
            &EventoOverlay::RatonMovido(Punto { x: 700, y: 600 }),
            &mut c,
            &Camara::nueva(),
            area,
            100,
        );
        assert!(!h.cambio && !c.sucia);
    }

    #[test]
    fn una_capa_rota_o_que_no_existe_empieza_vacia() {
        let dir =
            std::env::temp_dir().join(format!("pixpin-lector-tinta-rota-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("capa.excalidraw");
        assert!(Capa::leer(&ruta).vacia());
        std::fs::write(&ruta, "{ esto no es json").unwrap();
        assert!(Capa::leer(&ruta).vacia());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_hojas_del_pdf_se_nombran_desde_uno() {
        let r = ruta_de_hoja(Path::new("C:/planos/casa.pdf"), 0);
        assert_eq!(
            r.file_name().unwrap().to_string_lossy(),
            "hoja-1.excalidraw"
        );
        assert_eq!(
            r.parent().unwrap().file_name().unwrap().to_string_lossy(),
            "casa.pdf.pixpin-anotado"
        );
    }

    /// Un trazo a lapiz de 30 puntos que empieza en `(x, y)`, en una capa
    /// nueva: es el elemento 1 de su hoja, con la version que dan 30 puntos.
    fn hoja_con_un_trazo(x: f32, y: f32, ondas: f32) -> Capa {
        let mut c = Capa::default();
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        let puntos: Vec<Punto2> = (0..30)
            .map(|k| Punto2::nuevo(x + k as f32 * 6.0, y + ondas * (k as f32 / 3.0).sin()))
            .collect();
        t.trazar(&mut c, &puntos);
        c
    }

    fn ordenes_por_la_cache(
        cache: &mut Cache,
        e: &pixpin_motor2d::Elemento,
    ) -> Vec<pixpin_motor2d::pintado::Orden> {
        let mut v = Vec::new();
        crate::dibujo::pintar::por_cada_orden(cache, e, 1.0, None, |o| v.push(o.clone()));
        v
    }

    #[test]
    fn dos_hojas_con_un_elemento_del_mismo_id_y_version_pintan_cada_una_el_suyo() {
        let a = hoja_con_un_trazo(10.0, 10.0, 0.0);
        let b = hoja_con_un_trazo(400.0, 300.0, 40.0);
        let ea = a.escena.visibles().next().unwrap().clone();
        let eb = b.escena.visibles().next().unwrap().clone();
        // Lo que hace falta para que se confundan: mismo id y misma version.
        assert_eq!((ea.id, ea.version), (eb.id, eb.version));
        let (solo_a, solo_b) = (
            pixpin_motor2d::pintado::ordenes_a_distancia(&ea, 1.0),
            pixpin_motor2d::pintado::ordenes_a_distancia(&eb, 1.0),
        );
        assert_ne!(solo_a, solo_b);
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        // Como en cada fotograma del lector: una hoja y luego la otra.
        for _ in 0..3 {
            assert_eq!(ordenes_por_la_cache(t.geometria_de(0), &ea), solo_a);
            assert_eq!(
                ordenes_por_la_cache(t.geometria_de(1), &eb),
                solo_b,
                "la hoja 2 pinta lo suyo"
            );
        }
        // Caso negativo, el fallo de antes: con una cache para las dos, la
        // segunda hoja pinta la forma de la primera.
        let mut una = Cache::nueva();
        assert_eq!(ordenes_por_la_cache(&mut una, &ea), solo_a);
        assert_eq!(
            ordenes_por_la_cache(&mut una, &eb),
            solo_a,
            "una cache compartida confunde las hojas"
        );
    }

    #[test]
    fn pasado_el_tope_se_suelta_lo_calculado_de_la_hoja_que_lleva_mas_sin_verse() {
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        for hoja in 0..HOJAS_CALCULADAS + 4 {
            t.geometria_de(hoja);
        }
        assert_eq!(
            t.calculado.len(),
            HOJAS_CALCULADAS,
            "no crece con cada hoja que se ve"
        );
        // Las ultimas vistas siguen; la primera ya no.
        assert!(t.calculado.contains_key(&(HOJAS_CALCULADAS + 3)));
        assert!(!t.calculado.contains_key(&0));
        // Caso negativo: volver a una que sigue no suelta nada mas.
        t.geometria_de(HOJAS_CALCULADAS + 3);
        assert_eq!(t.calculado.len(), HOJAS_CALCULADAS);
        assert!(t.calculado.contains_key(&4));
    }

    /// Tres hojas con `n` trazos cada una (mitad lapiz, mitad grafito; los
    /// ids se repiten de una a otra) y un grafito en curso en la primera.
    fn tres_hojas_anotadas(t: &mut Tinta, n: usize) -> Vec<Capa> {
        let mut capas: Vec<Capa> = (0..3).map(|_| Capa::default()).collect();
        for (h, capa) in capas.iter_mut().enumerate() {
            for k in 0..n {
                let h_ = if k % 2 == 0 {
                    Herramienta::Lapiz
                } else {
                    Herramienta::Grafito
                };
                crate::dibujo::teclas::elegir_herramienta(&mut t.gesto, h_);
                let (x, y) = ((k * 37 % 1200) as f32, (k * 53 % 1800) as f32 + h as f32);
                let puntos: Vec<Punto2> = (0..40)
                    .map(|i| Punto2::nuevo(x + i as f32 * 4.0, y + 12.0 * (i as f32 / 4.0).sin()))
                    .collect();
                t.trazar(capa, &puntos);
            }
        }
        crate::dibujo::teclas::elegir_herramienta(&mut t.gesto, Herramienta::Grafito);
        let inicio: Vec<Punto2> = (0..20)
            .map(|i| Punto2::nuevo(100.0 + i as f32 * 3.0, 900.0))
            .collect();
        t.empezar_trazo(&mut capas[0], &inicio);
        t.hoja = Some(0);
        capas
    }

    /// **Anotar no rehace en cada fotograma lo que no cambio**, con muchas
    /// hojas anotadas: ni teselar de nuevo, ni subir bitmaps de grafito, ni
    /// recocerlos. Mide tambien el tiempo por fotograma con 1, 50 y 300
    /// trazos por hoja, y el de antes (todas las hojas con lo calculado de
    /// una sola, que era como iba), que es el caso negativo. Necesita GPU:
    /// `cargo test --release -p pixpin --bin pixpinmax anotar_no_rehace -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU"]
    fn anotar_no_rehace_en_cada_fotograma_lo_que_no_cambio() {
        use pixpin_motor2d::tinta::grafito;
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 1400, 1000).expect("superficie");
        let papel = ColorRgba::opaco(1.0, 1.0, 1.0);
        let vista = (0.0, 0.0, 1400.0, 2000.0);
        const FOTOGRAMAS: usize = 30;
        for n in [1usize, 50, 300] {
            for como_antes in [false, true] {
                let mut t = Tinta::sin_ajustes();
                let mut capas = tres_hojas_anotadas(&mut t, n);
                let fotograma = |t: &mut Tinta, capas: &mut [Capa], k: usize| {
                    // El trazo en curso crece un punto por fotograma.
                    t.seguir_trazo(
                        &mut capas[0],
                        Punto2::nuevo(160.0 + k as f32 * 3.0, 900.0 + (k % 7) as f32),
                    );
                    motor
                        .dibujar(&fuera.destino, |p| {
                            for (h, capa) in capas.iter().enumerate() {
                                let hoja = if como_antes { 0 } else { h };
                                t.pintar_capa(p, hoja, capa, vista, 0.5, papel, h == 0);
                            }
                        })
                        .expect("pintar");
                    fuera.esperar_gpu().expect("esperar");
                };
                // Dos para calentar: lo quieto se tesela y se sube una vez.
                fotograma(&mut t, &mut capas, 0);
                fotograma(&mut t, &mut capas, 1);
                let (r0, s0, c0) = (
                    t.realizaciones(),
                    t.subidas_de_grafito(),
                    grafito::cocciones(),
                );
                let reloj = std::time::Instant::now();
                for k in 2..2 + FOTOGRAMAS {
                    fotograma(&mut t, &mut capas, k);
                }
                let ms = reloj.elapsed().as_secs_f64() * 1000.0 / FOTOGRAMAS as f64;
                let (r, s, c) = (
                    t.realizaciones() - r0,
                    t.subidas_de_grafito() - s0,
                    grafito::cocciones() - c0,
                );
                println!(
                    "{n} trazos por hoja, {}: {ms:.2} ms/fotograma; en {FOTOGRAMAS} fotogramas: {r} teselados, {s} bitmaps de grafito subidos, {c} cocciones",
                    if como_antes {
                        "como antes (una cache)"
                    } else {
                        "una por hoja"
                    }
                );
                if !como_antes {
                    assert_eq!(r, 0, "con {n}: se teselo de nuevo lo que no cambio");
                    assert_eq!(s, 0, "con {n}: se subieron de nuevo bitmaps quietos");
                    assert_eq!(
                        c, 0,
                        "con {n}: se recocio grafito quieto (o el trazo en curso entero)"
                    );
                } else if n >= 50 {
                    // El caso negativo: asi iba, y es lo que se arregla.
                    assert!(
                        r + s + c > FOTOGRAMAS as u64,
                        "como antes deberia rehacer: {r} {s} {c}"
                    );
                }
            }
        }
    }

    #[test]
    fn escape_es_de_la_tinta_solo_si_hay_algo_que_soltar() {
        let mut t = tinta_de_prueba(Herramienta::Lapiz);
        assert!(
            !t.quiere_escape(),
            "sin nada, Escape es del lector: dejar de anotar"
        );
        t.gesto.seleccion.poner(7);
        assert!(t.quiere_escape());
    }
}
