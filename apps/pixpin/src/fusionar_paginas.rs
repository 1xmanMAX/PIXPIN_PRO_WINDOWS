//! **Varias paginas del PDF de un proyecto, juntas en un lienzo nuevo.**
//! Puerto de `motor/FusionDePaginas.kt` (las cuentas) y
//! `guardados/FusionarPaginas.kt` (el trabajo) del movil.
//!
//! Lo pidio el usuario (13-sep-2026): «fusionar paginas, por ejemplo la 4,
//! la 5 y la 6, en un mismo lienzo, separadas a una distancia razonable». Es
//! lo que se hace en papel cuando un detalle se entiende mirando dos planos a
//! la vez: se ponen en la mesa uno al lado del otro y se dibuja encima.
//!
//! La hoja que sale **es una mas del proyecto**, con un marco por pagina:
//! se comparte, se exporta y se sincroniza como cualquier otra, y al
//! entregarla en PDF vuelve a salir una pagina por plano. Las paginas van
//! como **imagen**, con el candado puesto (son el papel, no algo que se
//! dibuje): un lienzo tiene un solo papel debajo y aqui hay varios. La pagina
//! suelta sigue en el proyecto, vectorial y sin techo, para el detalle.
//!
//! En el PC se pide desde la tarjeta de Proyectos, que es donde lo pone el
//! movil (la caja de lo marcado): el clic derecho marca las paginas y, con
//! dos o mas, los tres puntos ofrecen «Fusionar en un lienzo». Las paginas
//! ya no salen en el chat (solo el mensaje del PDF); el menu del chat lo
//! sigue ofreciendo si se eligen paginas mandadas a mano. Se hace en su hilo
//! y el chat lo recoge al acabar.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_proyecto::{Hoja, Proyecto, almacen};

/// Mas de esto no se fusiona: son mapas de bits, y no cabrian.
pub const TOPE_DE_PAGINAS: usize = 12;
/// Los pixeles de todas las paginas juntas: 24 millones son ~96 MB mientras
/// se montan.
pub const PIXELES_EN_TOTAL: f64 = 24_000_000.0;
/// Ninguna pagina mas ancha que esto, por si se fusionan dos.
pub const ANCHO_MAXIMO: f64 = 2400.0;
/// Ni mas estrecha, aunque sean doce: por debajo no se lee.
pub const ANCHO_MINIMO: f64 = 700.0;
/// La separacion entre paginas, en tanto por uno del lado mas largo. Empezo
/// en 0,04 y el usuario las quiso **mas separadas** (14-sep-2026): casi
/// pegadas no se ve donde acaba una, y un trazo cruza sin querer.
pub const SEPARACION: f64 = 0.14;
/// La calidad del JPEG de cada pagina (la del movil).
const CALIDAD: u8 = 88;

/// Una pagina colocada: su sitio y lo que mide, en unidades del lienzo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sitio {
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
}

/// **En fila o en columna**: el conjunto mas cuadrado, que es el que mejor
/// se pasea. Verticales en fila, apaisadas en columna, y se decide con la
/// pagina media para que una portada apaisada no vuelque a las demas.
pub fn en_fila(medidas: &[(f64, f64)]) -> bool {
    if medidas.is_empty() {
        return true;
    }
    let mediana = |f: fn(&(f64, f64)) -> f64| {
        let mut v: Vec<f64> = medidas.iter().map(f).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    mediana(|m| m.1) >= mediana(|m| m.0)
}

/// La separacion entre paginas, en unidades del lienzo.
pub fn separacion(tamanos: &[(f64, f64)]) -> f64 {
    tamanos.iter().map(|t| t.0.max(t.1)).fold(0.0, f64::max) * SEPARACION
}

/// Donde cae cada pagina, en el orden en que llegan: en fila alineadas por
/// arriba, en columna por la izquierda.
pub fn sitios(tamanos: &[(f64, f64)], fila: bool) -> Vec<Sitio> {
    let hueco = separacion(tamanos);
    let (mut x, mut y) = (0.0, 0.0);
    tamanos
        .iter()
        .map(|&(an, al)| {
            let s = Sitio { x, y, ancho: an, alto: al };
            if fila {
                x += an + hueco;
            } else {
                y += al + hueco;
            }
            s
        })
        .collect()
}

/// **A cuantos pixeles de ancho se pinta cada pagina**, repartiendo
/// [`PIXELES_EN_TOTAL`] en proporcion a su papel. Los topes van a la escala
/// y no a cada pagina: recortando cada una, una del doble de grande salia
/// con la mitad de detalle que su vecina.
pub fn anchos(medidas: &[(f64, f64)]) -> Vec<u32> {
    if medidas.is_empty() {
        return Vec::new();
    }
    let area: f64 = medidas.iter().map(|m| m.0.max(1.0) * m.1.max(1.0)).sum();
    let mut k = if area > 0.0 { (PIXELES_EN_TOTAL / area).sqrt() } else { 1.0 };
    let mas_ancha = medidas.iter().map(|m| m.0.max(1.0)).fold(0.0, f64::max);
    let mas_estrecha = medidas.iter().map(|m| m.0.max(1.0)).fold(f64::MAX, f64::min);
    k = k.min(ANCHO_MAXIMO / mas_ancha);
    // Pasarse de pixeles se aguanta; no ver lo que pone, no.
    k = k.max(ANCHO_MINIMO / mas_estrecha);
    medidas.iter().map(|m| ((m.0.max(1.0) * k) as u32).max(1)).collect()
}

/// «3 a 5» si van seguidas; «3, 5 y 8» si no (desde 0 → desde 1).
fn cuales(paginas: &[u32]) -> String {
    let n: Vec<u32> = paginas.iter().map(|p| p + 1).collect();
    match n.as_slice() {
        [] => String::new(),
        [uno] => uno.to_string(),
        _ if n.windows(2).all(|w| w[1] == w[0] + 1) => format!("{} a {}", n[0], n[n.len() - 1]),
        [antes @ .., ultimo] => format!(
            "{} y {ultimo}",
            antes.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// **El nombre de la hoja fusionada**: que paginas son, porque lo que uno
/// busca en la lista es «donde esta lo de la 4» (usuario, 14-sep-2026).
pub fn nombre(paginas: &[u32]) -> String {
    match paginas {
        [] => "P\u{e1}ginas".into(),
        [una] => format!("P\u{e1}gina {}", una + 1),
        _ => format!("P\u{e1}ginas {}", cuales(paginas)),
    }
}

/// El rotulo del marco de una de sus paginas: «3 a 5 · pag. 4», con la
/// tilde del movil (va escrito en el proyecto y viaja al telefono). Con el
/// grupo delante se ve de un vistazo que van juntas.
pub fn rotulo(paginas: &[u32], pagina: u32) -> String {
    format!("{} \u{b7} p\u{e1}g. {}", cuales(paginas), pagina + 1)
}

// ---------------------------------------------------------------------------
// La peticion y el trabajo

/// Lo que hace falta saber de una fusion antes de pedirla.
#[derive(Clone, Debug, PartialEq)]
pub struct Peticion {
    pub ficha: String,
    pub pdf: PathBuf,
    /// Desde 0, sin repetir y en orden.
    pub paginas: Vec<u32>,
}

/// **Si lo elegido son dos o mas paginas del PDF del proyecto**, la peticion
/// para fusionarlas; `None` si no (hay un lienzo suelto, una nota, o una
/// sola pagina). `FusionarPaginas.de` del movil.
pub fn de(pdf: Option<PathBuf>, ficha: &str, elegidos: &[&Mensaje]) -> Option<Peticion> {
    let pdf = pdf.filter(|p| p.is_file())?;
    let mut paginas = Vec::new();
    for m in elegidos {
        // Un lienzo con marco, una nota o una tabla no son «la pagina».
        if ["marco", "nota", "tabla"].iter().any(|k| m.resto.get(*k).is_some_and(|v| !v.is_null())) {
            return None;
        }
        paginas.push(m.pagina?);
    }
    paginas.sort_unstable();
    paginas.dedup();
    if paginas.len() < 2 || paginas.len() > TOPE_DE_PAGINAS {
        return None;
    }
    Some(Peticion {
        ficha: ficha.to_string(),
        pdf,
        paginas,
    })
}

/// Una fusion acabada, para que el chat la recoja y la diga.
#[derive(Debug)]
pub struct Terminada {
    pub ficha: String,
    /// El nombre de la hoja nueva, o por que no salio.
    pub resultado: Result<String, String>,
}

fn terminadas_() -> &'static Mutex<Vec<Terminada>> {
    static T: OnceLock<Mutex<Vec<Terminada>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

/// Lo que ha terminado desde la ultima vez.
pub fn terminadas() -> Vec<Terminada> {
    terminadas_().lock().map(|mut t| std::mem::take(&mut *t)).unwrap_or_default()
}

/// **Empieza a montar el lienzo en su hilo**: son varias paginas pintadas, y
/// un plano grande tarda segundos en salir de Windows. `false` si no se pudo
/// lanzar.
pub fn empezar(raiz: &Path, peticion: Peticion) -> bool {
    let raiz = raiz.to_path_buf();
    std::thread::Builder::new()
        .name("pdf-fusionar".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let resultado = en_un_lienzo(&raiz, &peticion, ahora);
            if let Err(e) = &resultado {
                tracing::warn!(%e, "no se pudieron fusionar las paginas");
            }
            if let Ok(mut t) = terminadas_().lock() {
                t.push(Terminada {
                    ficha: peticion.ficha.clone(),
                    resultado,
                });
            }
            crate::pdf_en_chat::despertar();
        })
        .is_ok()
}

/// **Monta el lienzo y lo deja en el proyecto** (`enUnLienzo` del movil):
/// pinta cada pagina a su ancho, la guarda en `imagenes/` como JPEG, escribe
/// el `.excalidraw` con un marco y su pagina por cada una (las dos con
/// candado) y anade la hoja al proyecto. Devuelve el nombre de la hoja.
pub fn en_un_lienzo(raiz: &Path, peticion: &Peticion, ahora: i64) -> Result<String, String> {
    let doc = pixpin_pdf::Documento::abrir(&peticion.pdf).map_err(|e| e.to_string())?;
    let todas = doc.medidas();
    let medidas: Vec<(f64, f64)> = peticion
        .paginas
        .iter()
        .map(|&p| todas.get(p as usize).map_or((595.0, 842.0), |m| (m.0 as f64, m.1 as f64)))
        .collect();
    let anchos = anchos(&medidas);
    let carpeta = almacen::carpeta(raiz, &peticion.ficha);
    std::fs::create_dir_all(carpeta.join("imagenes")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(carpeta.join("lienzos")).map_err(|e| e.to_string())?;
    // Primero se pintan todas: la separacion sale de la mayor y no se sabe
    // hasta tenerlas.
    let mut hechas: Vec<(u32, String, f64, f64)> = Vec::new();
    for (i, &pagina) in peticion.paginas.iter().enumerate() {
        let img = match doc.renderizar(pagina, anchos[i]) {
            Ok(img) => img,
            Err(e) => {
                tracing::warn!(?e, pagina, "pagina que no se pudo pintar para fusionar");
                continue;
            }
        };
        let jpg = pixpin_codec::imagen::codificar_jpg(&img, CALIDAD).map_err(|e| e.to_string())?;
        let foto = format!("fus{ahora}x{i}");
        std::fs::write(carpeta.join("imagenes").join(&foto), jpg).map_err(|e| e.to_string())?;
        hechas.push((pagina, foto, img.ancho as f64, img.alto as f64));
    }
    if hechas.is_empty() {
        return Err("no salio ninguna pagina".into());
    }
    let tamanos: Vec<(f64, f64)> = hechas.iter().map(|h| (h.2, h.3)).collect();
    let sitios = sitios(&tamanos, en_fila(&tamanos));
    let dibujo = format!("dib-fus-{ahora}");
    let texto = lienzo(&peticion.paginas, &hechas, &sitios, ahora);
    std::fs::write(almacen::lienzo(raiz, &peticion.ficha, &dibujo), texto).map_err(|e| e.to_string())?;
    let nombre = nombre(&peticion.paginas);
    anadir_hoja(raiz, &peticion.ficha, &nombre, &dibujo, ahora)?;
    Ok(nombre)
}

/// El `.excalidraw` de la fusion: por cada pagina, su marco con su rotulo y
/// la foto de la pagina dentro, las dos con candado (lo pidio el usuario el
/// 14-sep-2026: son el papel, y un arrastre sin querer movia la pagina con
/// lo ya dibujado encima).
fn lienzo(paginas: &[u32], hechas: &[(u32, String, f64, f64)], sitios: &[Sitio], ahora: i64) -> String {
    let mut elementos = Vec::new();
    let mut ficheros = serde_json::Map::new();
    for (i, ((pagina, foto, _, _), s)) in hechas.iter().zip(sitios).enumerate() {
        let comun = |id: String, tipo: &str, semilla: i64| {
            serde_json::json!({
                "id": id, "type": tipo,
                "x": s.x, "y": s.y, "width": s.ancho, "height": s.alto,
                "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
                "roughness": 0, "opacity": 100, "groupIds": [], "frameId": null,
                "seed": semilla, "version": 1, "versionNonce": 1, "isDeleted": false,
                "boundElements": null, "locked": true
            })
        };
        let mut marco = comun(format!("marco-fus-{ahora}-{i}"), "frame", 1);
        marco["name"] = serde_json::Value::String(rotulo(paginas, *pagina));
        let mut imagen = comun(format!("pag-fus-{ahora}-{i}"), "image", 2);
        imagen["fileId"] = serde_json::Value::String(foto.clone());
        imagen["status"] = "saved".into();
        imagen["scale"] = serde_json::json!([1, 1]);
        imagen["strokeColor"] = "transparent".into();
        elementos.push(marco);
        elementos.push(imagen);
        ficheros.insert(
            foto.clone(),
            serde_json::json!({"id": foto, "mimeType": "image/jpeg", "path": format!("imagenes/{foto}"), "created": ahora}),
        );
    }
    serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-max",
        "elements": elementos,
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": ficheros
    })
    .to_string()
}

/// Anade la hoja al `proyecto.json`, de un tiron (al lado y luego el nombre).
fn anadir_hoja(raiz: &Path, ficha: &str, nombre: &str, dibujo: &str, ahora: i64) -> Result<(), String> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let ruta = carpeta.join("proyecto.json");
    let mut p: Proyecto = std::fs::read_to_string(&ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("el proyecto no se lee")?;
    let mut h = Hoja {
        id: format!("hoja-fus-{ahora}"),
        nombre: nombre.to_string(),
        dibujo: Some(dibujo.to_string()),
        ..Default::default()
    };
    // Con su codigo unico: es lo que la hace salir en el chat y en la
    // galeria, y lo que la sincronizacion reconoce.
    h.uid = Some(h.codigo_unico());
    p.hojas.push(h);
    p.tocado = ahora;
    let temporal = carpeta.join("proyecto.json.fus.tmp");
    std::fs::write(&temporal, serde_json::to_string(&p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&temporal, &ruta).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "fusionar_paginas/pruebas.rs"]
mod pruebas;
