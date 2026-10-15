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
    /// El chat (su ficha, `proyectos/<ficha>`) donde vive lo que se ve, si
    /// se sabe: su `.pixpin` es ese proyecto entero. Para lo que vive en un
    /// chat sin ser un adjunto con ruta (una leccion, un croquis 3D).
    pub proyecto: Option<String>,
    /// Lo anotado que va con el original en un `.pixpin` nuevo (ficheros
    /// que viven al lado o en la cache).
    pub anotado: Vec<PathBuf>,
    pub hojas: Vec<HojaSuelta>,
    /// Las hojas que son tablas (por su numero en `hojas`): su HTML es la
    /// tabla que sigue calculando, no su dibujo.
    pub tablas: HashMap<usize, pixpin_proyecto::tabla::Tabla>,
    pub propios: Vec<Propio>,
}

impl Exportable {
    pub(crate) fn nuevo(titulo: impl Into<String>) -> Exportable {
        Exportable { titulo: titulo.into(), raiz: None, original: None, proyecto: None, anotado: Vec::new(), hojas: Vec::new(), tablas: HashMap::new(), propios: Vec::new() }
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

/// **El `.pixpin` de un fichero suelto**: si se sabe su chat (`proyecto`) o
/// es un adjunto de un chat, el proyecto entero de ese chat (lo que manda el
/// chat); si no, un proyecto nuevo con el fichero como mensaje y lo anotado
/// al lado (`archivos/<nombre>` y `archivos/<nombre>.anotado/...`).
pub(crate) fn pixpin_de(raiz: Option<&Path>, proyecto: Option<&str>, original: &Path, anotado: &[PathBuf], cuando: i64) -> Result<(String, Vec<u8>)> {
    let nombre = original.file_name().map(|n| n.to_string_lossy().into_owned()).context("el fichero no tiene nombre")?;
    let tronco = pixpin_docs::sin_extension(&nombre);
    if let (Some(raiz), Some(id)) = (raiz, proyecto)
        && let Ok(bytes) = pixpin_proyecto::almacen::empaquetar(raiz, id, cuando)
    {
        let ficha = pixpin_proyecto::almacen::Indice::leer(raiz).buscar(id).map(|f| f.nombre.clone()).unwrap_or(tronco.clone());
        return Ok((format!("{}.pixpin", super::nombre_de_fichero(&ficha)), bytes));
    }
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

/// Lo que va como un mensaje en un `.pixpin` nuevo ([`pixpin_nuevo`]): su
/// texto y sus ficheros (fotos, audios...), cada uno su mensaje.
#[derive(Debug, Clone)]
pub(crate) struct Apunte {
    pub cuando: i64,
    pub texto: String,
    pub ficheros: Vec<PathBuf>,
}

/// **Un `.pixpin` nuevo con unos apuntes** (lo que no vive en un solo chat:
/// los momentos de la timeline, unas lecciones de varios chats): un proyecto
/// llamado `titulo` con cada texto como nota y cada fichero como su mensaje
/// (imagen, voz o archivo), en `archivos/`. Lo que falta del disco se salta.
pub(crate) fn pixpin_nuevo(titulo: &str, apuntes: &[Apunte], cuando: i64) -> Result<(String, Vec<u8>)> {
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};
    anyhow::ensure!(!apuntes.is_empty(), "no hay nada que poner en el .pixpin");
    let nombre = if titulo.trim().is_empty() { "PixPin".to_string() } else { titulo.trim().to_string() };
    let proyecto = pixpin_proyecto::Proyecto { id: format!("exportado-{cuando}"), nombre: nombre.clone(), creado: cuando, tocado: cuando, ..Default::default() };
    let manifiesto = pixpin_proyecto::Manifiesto { escrito: cuando, proyecto: nombre.clone(), ..Default::default() };
    let mut p = pixpin_proyecto::Paquete::nuevo(manifiesto, proyecto);
    let mut lineas = String::new();
    let mut usados = std::collections::HashSet::new();
    let mut n = 0u32;
    let mut poner = |m: Mensaje, lineas: &mut String| -> Result<()> {
        lineas.push_str(&serde_json::to_string(&m).context("el mensaje no se escribe")?);
        lineas.push('\n');
        Ok(())
    };
    for a in apuntes {
        if !a.texto.trim().is_empty() {
            n += 1;
            let m = Mensaje { id: format!("m-{cuando}-{n}"), cuando: a.cuando, clase: Some(Clase::Nota), texto: a.texto.clone(), ..Default::default() };
            poner(m, &mut lineas)?;
        }
        for f in a.ficheros.iter().filter(|f| f.is_file()) {
            let Ok(bytes) = std::fs::read(f) else { continue };
            let base = f.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "fichero".into());
            // Dos ficheros con el mismo nombre no se pisan.
            let mut fichero = base.clone();
            let mut k = 1;
            while !usados.insert(fichero.clone()) {
                k += 1;
                fichero = format!("{k}-{base}");
            }
            let ext = f.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let clase = match ext.as_str() {
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Clase::Imagen,
                "m4a" | "ogg" | "opus" | "wav" | "mp3" | "aac" => Clase::Voz,
                _ => Clase::Archivo,
            };
            let ruta = format!("archivos/{fichero}");
            n += 1;
            let m = Mensaje { id: format!("m-{cuando}-{n}"), cuando: a.cuando, clase: Some(clase), ruta: Some(ruta.clone()), nombre: fichero.clone(), bytes: bytes.len() as i64, ..Default::default() };
            p.poner_entrada(&ruta, bytes);
            poner(m, &mut lineas)?;
        }
    }
    anyhow::ensure!(!lineas.is_empty(), "no hay nada que poner en el .pixpin");
    p.poner_entrada("guardados.jsonl", lineas.into_bytes());
    let bytes = p.a_bytes().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((format!("{}.pixpin", super::nombre_de_fichero(&nombre)), bytes))
}

/// El `.pixpin` de [`pixpin_nuevo`] como formato propio de un visor.
pub(crate) fn propio_pixpin(titulo: String, apuntes: Vec<Apunte>) -> Propio {
    Propio::nuevo(PIXPIN, "compartir-pixpin", move |c| {
        let (fichero, bytes) = pixpin_nuevo(&titulo, &apuntes, pixpin_shell::entorno::ahora_utc_ms())?;
        let r = c.join(fichero);
        std::fs::write(&r, bytes).with_context(|| format!("no se pudo escribir {}", r.display()))?;
        Ok(vec![r])
    })
}

/// **Una imagen con lo dibujado encima** (un pin, una captura, una foto):
/// la hoja del motor con la imagen de papel y su `.pixpin2d`; el original,
/// la imagen tal cual.
pub(crate) fn de_imagen(titulo: &str, imagen: &Path, dibujo: Option<&Path>) -> Result<Exportable> {
    use pixpin_motor2d::exportar::{self as ex, Alcance, ID_PAPEL};
    let img = pixpin_codec::cargar(imagen).with_context(|| format!("no se pudo leer {}", imagen.display()))?;
    let (w, h) = (img.ancho as f32, img.alto as f32);
    let escena = dibujo.and_then(|d| pixpin_motor2d::cargar(d).ok()).unwrap_or_else(pixpin_motor2d::Escena::nueva);
    let hoja = ex::hojas(&escena, Alcance::Todo, &[], Some((w, h))).into_iter().next().context("sin hoja")?;
    let mut x = Exportable::nuevo(titulo);
    x.original = Some(imagen.to_path_buf());
    if let Some(d) = dibujo.filter(|d| d.is_file()) {
        x.anotado.push(d.to_path_buf());
    }
    x.hojas.push(HojaSuelta { nombre: String::new(), hoja, fondo: super::BLANCO, imagenes: HashMap::from([(ID_PAPEL, img)]) });
    Ok(x)
}

/// **Las hojas de lo que se imprime** (`imprimir_moderno::Documento`: un
/// Word o un PowerPoint con hoja, un plano...): cada hoja pintada como en el
/// papel, con lo anotado, a `escala` pixeles por DIP. Sirve a todo visor que
/// ya sabe imprimir: de esas hojas salen PDF, HTML, PNG, JPG y SVG.
pub(crate) fn hojas_impresas(doc: &mut dyn pixpin_render::imprimir_moderno::Documento, papel: (f32, f32), escala: f32, tope: usize) -> Result<Vec<HojaSuelta>> {
    use pixpin_motor2d::pintado::Orden;
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para exportar")?;
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).context("sin motor para exportar")?;
    let n = doc.paginar(&motor, None).min(tope);
    let (w, h) = ((papel.0 * escala).round().max(1.0) as u32, (papel.1 * escala).round().max(1.0) as u32);
    let mut hojas = Vec::with_capacity(n);
    for i in 0..n {
        let destino = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h).context("sin textura para exportar")?;
        motor
            .dibujar(&destino.destino, |p| {
                p.limpiar(pixpin_render::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
                doc.pintar(i, pixpin_render::imprimir_moderno::Pagina { papel, escala }, p);
            })
            .context("no se pudo pintar la hoja")?;
        let (ancho, alto, pixeles) = destino.leer_rgba().context("no se pudo leer la hoja")?;
        let orden = Orden::Imagen { id_objeto: 1, x: 0.0, y: 0.0, ancho: papel.0, alto: papel.1, opacidad: 1.0, recorte: None, angulo: 0.0 };
        let hoja = Hoja { nombre: String::new(), caja: (0.0, 0.0, papel.0, papel.1), ordenes: vec![orden], marcos: Vec::new(), granos: Vec::new(), grafitos: Vec::new() };
        hojas.push(HojaSuelta { nombre: String::new(), hoja, fondo: super::BLANCO, imagenes: HashMap::from([(1, ImagenRgba { ancho, alto, pixeles })]) });
    }
    Ok(hojas)
}

/// Una imagen ya hecha como hoja (la vista de un visor 3D, una hoja impresa).
pub(crate) fn hoja_de_imagen(img: ImagenRgba) -> HojaSuelta {
    use pixpin_motor2d::pintado::Orden;
    let (w, h) = (img.ancho as f32, img.alto as f32);
    let orden = Orden::Imagen { id_objeto: 1, x: 0.0, y: 0.0, ancho: w, alto: h, opacidad: 1.0, recorte: None, angulo: 0.0 };
    let hoja = Hoja { nombre: String::new(), caja: (0.0, 0.0, w, h), ordenes: vec![orden], marcos: Vec::new(), granos: Vec::new(), grafitos: Vec::new() };
    HojaSuelta { nombre: String::new(), hoja, fondo: super::BLANCO, imagenes: HashMap::from([(1, img)]) }
}

/// **El «Exportar…» de un visor 3D** (modelo BIM, croquis): la vista que se
/// ve como hoja (PDF, HTML, PNG, JPG, SVG) y, si hay fichero, el original y
/// el `.pixpin` (el de su chat, `proyecto`, si se sabe: un croquis).
pub(crate) fn al_exportar_3d(idioma: pixpin_store::Idioma, titulo: String, original: Option<PathBuf>, proyecto: Option<String>) -> pixpin_cad::ventana3d::AlExportar {
    let texto = pixpin_store::Catalogo::nuevo(idioma).t("compartir-exportar") + "\tCtrl+E";
    pixpin_cad::ventana3d::AlExportar {
        texto,
        hacer: Arc::new(move |foto: Option<(u32, u32, Vec<u8>)>| {
            let Some(ubicacion) = crate::oscuro::ubicacion() else { return };
            let mut x = Exportable::nuevo(titulo.clone());
            x.raiz = Some(ubicacion.raiz().to_path_buf());
            x.original = original.clone().filter(|o| o.is_file());
            x.proyecto = proyecto.clone();
            if let Some((ancho, alto, pixeles)) = foto {
                x.hojas.push(hoja_de_imagen(ImagenRgba { ancho, alto, pixeles }));
            }
            super::ventana::abrir(idioma, ubicacion, super::Cosa::Visor(x));
        }),
    }
}

/// **Un libro de hojas de calculo** (xlsx, ods, csv): cada hoja como tabla
/// (su HTML calcula en el navegador; PDF e imagen, su rejilla) y, propios,
/// el CSV de cada hoja; el original es el xlsx tal cual.
pub(crate) fn de_libro(titulo: &str, ruta: &Path) -> Result<Exportable> {
    let nombre = pixpin_docs::nombre(ruta);
    let hojas = pixpin_proyecto::importar_hojas::leer(ruta, &nombre, pixpin_shell::entorno::ahora_utc_ms()).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut x = Exportable::nuevo(titulo);
    x.original = Some(ruta.to_path_buf());
    let mut csv = Vec::new();
    for h in hojas {
        if let Some(hoja) = super::hoja_de_tabla(&h.tabla) {
            x.tablas.insert(x.hojas.len(), h.tabla.clone());
            x.hojas.push(HojaSuelta { nombre: h.nombre.clone(), hoja, fondo: super::BLANCO, imagenes: HashMap::new() });
        }
        csv.push((h.nombre, super::csv_de(&h.tabla)));
    }
    anyhow::ensure!(!x.hojas.is_empty() || !csv.is_empty(), "el libro no tiene hojas");
    let base = super::nombre_de_fichero(titulo);
    x.propios.push(Propio::nuevo("csv", "compartir-csv", move |c| {
        let mut v = Vec::new();
        for (n, t) in &csv {
            let r = if csv.len() == 1 { c.join(format!("{base}.csv")) } else { c.join(format!("{base} - {}.csv", super::nombre_de_fichero(n))) };
            std::fs::write(&r, t).with_context(|| format!("no se pudo escribir {}", r.display()))?;
            v.push(r);
        }
        Ok(v)
    }));
    Ok(x)
}

/// **Varias imagenes** (capturas de la galeria): una hoja cada una; con una
/// sola, tambien su original y su `.pixpin`.
pub(crate) fn de_imagenes(titulo: &str, rutas: &[PathBuf]) -> Result<Exportable> {
    let mut x = Exportable::nuevo(titulo);
    for r in rutas {
        let nombre = r.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let y = de_imagen(&nombre, r, None)?;
        for mut h in y.hojas {
            h.nombre = nombre.clone();
            x.hojas.push(h);
        }
    }
    anyhow::ensure!(!x.hojas.is_empty(), "no hay imagenes");
    if let [una] = rutas {
        x.original = Some(una.clone());
    }
    Ok(x)
}

/// **Un texto** (una nota): sus paginas A4 y, propios, el `.txt` y el `.md`.
pub(crate) fn de_texto(titulo: &str, texto: &str, original: Option<&Path>) -> Exportable {
    let mut x = Exportable::nuevo(titulo);
    x.original = original.map(Path::to_path_buf);
    for hoja in super::hojas_de_texto(None, texto) {
        x.hojas.push(HojaSuelta { nombre: String::new(), hoja, fondo: super::BLANCO, imagenes: HashMap::new() });
    }
    let base = super::nombre_de_fichero(titulo);
    for (id, clave, ext) in [("texto", "compartir-texto", "txt"), ("md", "compartir-md", "md")] {
        let (t, b) = (texto.to_string(), base.clone());
        x.propios.push(Propio::nuevo(id, clave, move |c| {
            let r = c.join(format!("{b}.{ext}"));
            std::fs::write(&r, &t).with_context(|| format!("no se pudo escribir {}", r.display()))?;
            Ok(vec![r])
        }));
    }
    x
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-exportable-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Lo minimo del usuario: PDF, HTML, .pixpin, imagen y original.
    fn tiene_lo_minimo(f: &[&str]) {
        for id in [super::super::PDF, super::super::WEB, PIXPIN, super::super::PNG, ORIGINAL] {
            assert!(f.contains(&id), "falta {id}: {f:?}");
        }
    }

    #[test]
    fn una_imagen_y_una_nota_ofrecen_lo_minimo() {
        let d = temporal("minimo");
        let foto = d.join("captura.png");
        std::fs::write(&foto, pixpin_codec::codificar_png(&ImagenRgba { ancho: 4, alto: 3, pixeles: vec![200; 48] }).unwrap()).unwrap();
        tiene_lo_minimo(&de_imagen("captura", &foto, None).unwrap().formatos());
        let nota = d.join("nota.txt");
        std::fs::write(&nota, "hola").unwrap();
        let x = de_texto("nota", "hola", Some(&nota));
        tiene_lo_minimo(&x.formatos());
        assert!(x.formatos().contains(&"md") && x.formatos().contains(&"texto"));
        // Caso negativo: una imagen que no esta no se exporta.
        assert!(de_imagen("x", &d.join("no-esta.png"), None).is_err());
        // Y sin fichero no hay ni original ni .pixpin.
        assert!(!de_texto("x", "y", None).formatos().contains(&PIXPIN));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_pixpin_de_un_fichero_suelto_se_vuelve_a_abrir_con_lo_anotado() {
        let d = temporal("pixpin");
        let f = d.join("plano.dxf");
        std::fs::write(&f, "0\nEOF\n").unwrap();
        let a = d.join("plano.anot.excalidraw");
        std::fs::write(&a, "{}").unwrap();
        let (nombre, bytes) = pixpin_de(None, None, &f, std::slice::from_ref(&a), 1_700_000_000_000).unwrap();
        assert_eq!(nombre, "plano.pixpin");
        let p = pixpin_proyecto::Paquete::desde_bytes(&bytes).unwrap();
        assert_eq!(p.proyecto.nombre, "plano");
        assert_eq!(p.entrada("archivos/plano.dxf"), Some(&b"0\nEOF\n"[..]));
        assert!(p.entrada("archivos/plano.dxf.anotado/plano.anot.excalidraw").is_some());
        assert!(String::from_utf8_lossy(p.entrada("guardados.jsonl").unwrap()).contains("archivos/plano.dxf"));
        // Caso negativo: un fichero que no esta.
        assert!(pixpin_de(None, None, &d.join("no-esta.dwg"), &[], 1).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_pixpin_nuevo_lleva_cada_apunte_y_sus_ficheros() {
        let d = temporal("nuevo");
        let foto = d.join("foto.png");
        std::fs::write(&foto, pixpin_codec::codificar_png(&ImagenRgba { ancho: 2, alto: 2, pixeles: vec![9; 16] }).unwrap()).unwrap();
        let otra = d.join("sub");
        std::fs::create_dir_all(&otra).unwrap();
        std::fs::write(otra.join("foto.png"), b"no es png pero da igual").unwrap();
        let apuntes = vec![
            Apunte { cuando: 1, texto: "Vaciado de losa".into(), ficheros: vec![foto.clone(), otra.join("foto.png")] },
            // Lo que falta del disco se salta.
            Apunte { cuando: 2, texto: String::new(), ficheros: vec![d.join("no-esta.m4a")] },
        ];
        let (nombre, bytes) = pixpin_nuevo("Timeline: hoy", &apuntes, 7).unwrap();
        assert!(nombre.ends_with(".pixpin"), "{nombre}");
        let p = pixpin_proyecto::Paquete::desde_bytes(&bytes).unwrap();
        assert_eq!(p.proyecto.nombre, "Timeline: hoy");
        assert!(p.entrada("archivos/foto.png").is_some() && p.entrada("archivos/2-foto.png").is_some());
        let lineas = String::from_utf8_lossy(p.entrada("guardados.jsonl").unwrap()).into_owned();
        assert_eq!(lineas.lines().count(), 3, "{lineas}");
        assert!(lineas.contains("Vaciado de losa") && lineas.contains("IMAGEN"));
        // Casos negativos: sin apuntes, o solo con lo que no esta, no hay .pixpin.
        assert!(pixpin_nuevo("x", &[], 1).is_err());
        assert!(pixpin_nuevo("x", &apuntes[1..], 1).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_menu_comun_usa_el_formato_propio_y_genera_cada_uno() {
        let d = temporal("menu");
        let foto = d.join("captura.png");
        std::fs::write(&foto, pixpin_codec::codificar_png(&ImagenRgba { ancho: 40, alto: 30, pixeles: vec![90; 4800] }).unwrap()).unwrap();
        let mut x = de_imagen("captura", &foto, None).unwrap();
        // Un HTML propio sustituye al de las hojas.
        x.propios.push(Propio::nuevo(super::super::WEB, "compartir-web", |c| {
            let r = c.join("propio.html");
            std::fs::write(&r, "<!DOCTYPE html><title>propio</title>")?;
            Ok(vec![r])
        }));
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let p = super::super::preparar(super::super::Cosa::Visor(x), &t).unwrap();
        let c = super::super::compartible(&p, &t);
        let ids: Vec<&str> = c.formatos.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids.iter().filter(|i| **i == "web").count(), 1, "{ids:?}");
        let claves: Vec<String> = p.piezas.iter().map(|x| x.pagina.clave.clone()).collect();
        let salida = d.join("salida");
        for id in ["web", "pdf", "png", "jpg", "svg", "original", "pixpin"] {
            let s = super::super::generar(&p, id, &claves, &salida).unwrap_or_else(|e| panic!("{id}: {e:#}"));
            assert!(s.ficheros.iter().all(|f| f.is_file()) && s.bytes > 0, "{id}");
            if id == "web" {
                assert!(std::fs::read_to_string(&s.ficheros[0]).unwrap().contains("propio"));
            }
            if id == "pdf" {
                assert!(std::fs::read(&s.ficheros[0]).unwrap().starts_with(b"%PDF"));
            }
        }
        // Caso negativo: un formato que nadie sabe hacer.
        assert!(super::super::generar(&p, "xyz", &claves, &salida).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
