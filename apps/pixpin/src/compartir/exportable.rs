//! **Exportar desde cualquier visor, con un solo componente** (13-oct-2026,
//! regla del usuario: «un solo componente Exportar comun, un menu que todos
//! usan, al que cada visor le pasa la lista de formatos que sabe hacer y una
//! funcion para cada uno; no un exportador copiado en cada visor»).
//!
//! El componente es la hoja de compartir de siempre ([`super::ventana`]):
//! sus salidas (Guardar como…, Copiar, el panel Compartir de Windows, la
//! wifi y el enlace), su peso calculado antes de pulsar y sus paginas. Un
//! visor le da un [`Exportable`]:
//!
//! - **sus hojas** (`pixpin_motor2d::exportar::Hoja`, lo que ya sabe pintar
//!   con lo anotado encima), de las que la hoja saca sola PDF, HTML, PNG,
//!   JPG y SVG como para un lienzo;
//! - **sus formatos propios**, cada uno con la funcion que lo hace
//!   ([`Propio`]): uno con el mismo id que uno de los de las hojas («web»,
//!   «pdf», «png»…) lo sustituye (el HTML interactivo de un plano, el PDF en
//!   vectores...);
//! - **el fichero que se esta viendo**, del que salen siempre «Original»
//!   (la copia tal cual) y **«.pixpin»** (el proyecto del chat donde vive, o
//!   uno nuevo con el fichero dentro y lo anotado al lado).
//!
//! Lo comun (Original y .pixpin) lo pone aqui la hoja, no cada visor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::exportar::Hoja;

/// La funcion que hace un formato: escribe sus ficheros en la carpeta que
/// se le da y dice cuales son.
pub(crate) type Hacer = Arc<dyn Fn(&Path) -> Result<Vec<PathBuf>> + Send + Sync>;

/// Un formato que el visor sabe hacer el solo.
#[derive(Clone)]
pub(crate) struct Propio {
    /// El id del formato (`web`, `pdf`, `png`, `dxf`, `csv`...): con el de
    /// un formato de las hojas, lo sustituye.
    pub id: &'static str,
    /// La clave de su nombre en el catalogo (`compartir-web`...).
    pub clave: &'static str,
    pub hacer: Hacer,
}

impl Propio {
    pub(crate) fn nuevo(id: &'static str, clave: &'static str, hacer: impl Fn(&Path) -> Result<Vec<PathBuf>> + Send + Sync + 'static) -> Propio {
        Propio { id, clave, hacer: Arc::new(hacer) }
    }
}

/// Una hoja ya pintada por el visor, con los pixeles de sus imagenes.
pub(crate) struct HojaSuelta {
    pub nombre: String,
    pub hoja: Hoja,
    pub fondo: ColorRgba,
    pub imagenes: HashMap<u64, ImagenRgba>,
}

/// **Lo que un visor ofrece para exportar.**
pub(crate) struct Exportable {
    pub titulo: String,
    /// La carpeta de datos de PixPin (para el `.pixpin` del proyecto).
    pub raiz: Option<PathBuf>,
    /// El fichero que se ve: da «Original» y «.pixpin».
    pub original: Option<PathBuf>,
    /// Lo anotado que va con el original en un `.pixpin` nuevo (ficheros
    /// que viven al lado o en la cache).
    pub anotado: Vec<PathBuf>,
    pub hojas: Vec<HojaSuelta>,
    pub propios: Vec<Propio>,
}

impl Exportable {
    pub(crate) fn nuevo(titulo: impl Into<String>) -> Exportable {
        Exportable { titulo: titulo.into(), raiz: None, original: None, anotado: Vec::new(), hojas: Vec::new(), propios: Vec::new() }
    }

    /// Los ids de los formatos que ofrecera la hoja (para las pruebas y el
    /// inventario): los de las hojas, los propios y los comunes.
    pub(crate) fn formatos(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Vec::new();
        let mut poner = |id: &'static str| {
            if !v.contains(&id) {
                v.push(id);
            }
        };
        if !self.hojas.is_empty() {
            for id in [super::WEB, super::PDF, super::PNG, super::JPG, super::SVG] {
                poner(id);
            }
        }
        for p in &self.propios {
            poner(p.id);
        }
        if self.original.is_some() {
            poner(ORIGINAL);
            poner(PIXPIN);
        }
        v
    }
}

pub(crate) const ORIGINAL: &str = "original";
pub(crate) const PIXPIN: &str = "pixpin";

/// **El `.pixpin` de un fichero suelto**: si es un adjunto de un chat, el
/// proyecto entero de ese chat (lo que manda el chat); si no, un proyecto
/// nuevo con el fichero como mensaje y lo anotado al lado
/// (`archivos/<nombre>` y `archivos/<nombre>.anotado/...`).
pub(crate) fn pixpin_de(raiz: Option<&Path>, original: &Path, anotado: &[PathBuf], cuando: i64) -> Result<(String, Vec<u8>)> {
    let nombre = original.file_name().map(|n| n.to_string_lossy().into_owned()).context("el fichero no tiene nombre")?;
    let tronco = pixpin_docs::sin_extension(&nombre);
    if let Some(raiz) = raiz
        && let Some(r) = pixpin_proyecto::anotado::raiz_de(original).or_else(|| Some(raiz.to_path_buf()))
        && let Some(x) = pixpin_proyecto::anotado::adjunto_de(&r, original)
        && let Ok(bytes) = pixpin_proyecto::almacen::empaquetar(&r, &x.chat, cuando)
    {
        let ficha = pixpin_proyecto::almacen::Indice::leer(&r).buscar(&x.chat).map(|f| f.nombre.clone()).unwrap_or(tronco.clone());
        return Ok((format!("{}.pixpin", super::nombre_de_fichero(&ficha)), bytes));
    }
    let bytes = std::fs::read(original).with_context(|| format!("no se pudo leer {}", original.display()))?;
    let id = format!("exportado-{cuando}");
    let proyecto = pixpin_proyecto::Proyecto { id: id.clone(), nombre: tronco.clone(), creado: cuando, tocado: cuando, ..Default::default() };
    let manifiesto = pixpin_proyecto::Manifiesto { escrito: cuando, proyecto: tronco.clone(), ..Default::default() };
    let mut p = pixpin_proyecto::Paquete::nuevo(manifiesto, proyecto);
    let ruta = format!("archivos/{nombre}");
    let mensaje = pixpin_proyecto::cuaderno::Mensaje {
        id: format!("m-{cuando}"),
        cuando,
        clase: Some(pixpin_proyecto::cuaderno::Clase::Archivo),
        ruta: Some(ruta.clone()),
        nombre: nombre.clone(),
        bytes: bytes.len() as i64,
        ..Default::default()
    };
    p.poner_entrada(&ruta, bytes);
    for a in anotado.iter().filter(|a| a.is_file()) {
        if let (Some(n), Ok(b)) = (a.file_name(), std::fs::read(a)) {
            p.poner_entrada(&format!("archivos/{nombre}.anotado/{}", n.to_string_lossy()), b);
        }
    }
    let linea = serde_json::to_string(&mensaje).context("el mensaje no se escribe")?;
    p.poner_entrada("guardados.jsonl", format!("{linea}\n").into_bytes());
    let bytes = p.a_bytes().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((format!("{}.pixpin", super::nombre_de_fichero(&tronco)), bytes))
}
