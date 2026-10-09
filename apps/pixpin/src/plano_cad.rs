//! **Ver planos DWG y DXF** (8-oct-2026): el usuario pidio un visor «ligero,
//! preciso y fluido aunque el plano sea grande y el equipo modesto», que se
//! abra como un pin. El visor esta en `pixpin-cad`; aqui, como se abre desde
//! PixPin.
//!
//! **El plano se lee en otro proceso** (`pixpinmax.exe --cad-convertir
//! <plano> <cache>`): la libreria que lee los DWG se defiende de un fichero
//! roto con `catch_unwind`, y el ejecutable va con `panic = "abort"`; asi un
//! plano dañado solo tumba a ese proceso, no a PixPin. De paso, la memoria
//! que usa la lectura se devuelve entera al terminar, y lo leido queda en
//! `<raiz>/cache/cad/`: abrir otra vez el mismo plano es instantaneo.

use std::path::{Path, PathBuf};

use pixpin_store::{Idioma, Ubicacion};

/// Imprimir los marcos del plano, con su lamina y su membrete.
mod imprimir;
/// Lo anotado sobre el plano (la capa anot-<uid>).
mod anotar;

/// La orden con que PixPin se llama a si misma para leer un plano.
pub const ORDEN: &str = "--cad-convertir";

/// Cuantos planos se guardan ya leidos (los mas recientes).
const EN_CACHE: usize = 24;

pub fn se_abre(nombre: &str) -> bool {
    matches!(pixpin_docs::extension(nombre).as_str(), "dwg" | "dxf")
}

/// Si PixPin se lanzo para leer un plano, lo lee y dice con que codigo
/// salir. `None`: es un arranque normal.
pub fn convertir_si_toca() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != ORDEN) {
        return None;
    }
    let (Some(entrada), Some(salida)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    Some(match pixpin_cad::convertir::convertir_fichero(&entrada) {
        Ok((modelo, _)) => {
            let temporal = salida.with_extension("tmp");
            let escrito = std::fs::write(&temporal, modelo.a_bytes()).and_then(|()| std::fs::rename(&temporal, &salida));
            if escrito.is_ok() { 0 } else { 3 }
        }
        Err(e) => {
            let _ = std::fs::write(salida.with_extension("error"), e);
            1
        }
    })
}

/// El nombre del plano en la cache: su ruta, su tamano y su fecha.
pub fn clave(ruta: &Path) -> Option<String> {
    let m = std::fs::metadata(ruta).ok()?;
    let fecha = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
    let texto = format!("{}|{}|{}", ruta.display(), m.len(), fecha);
    // FNV-1a de 64 bits: basta para no confundir dos planos.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{h:016x}"))
}

/// Deja solo los `EN_CACHE` planos mas recientes.
fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else { return };
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = d
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in v.into_iter().skip(EN_CACHE * 2) {
        let _ = std::fs::remove_file(p);
    }
}

/// Lee el plano: de la cache, o en el proceso aparte.
fn cargar(raiz: &Path, ruta: &Path) -> Result<pixpin_cad::modelo::Modelo, String> {
    let carpeta = raiz.join("cache").join("cad");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let clave = clave(ruta).ok_or("no se puede leer el fichero")?;
    let destino = carpeta.join(format!("{clave}.pxcad"));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(m) = pixpin_cad::modelo::Modelo::de_bytes(&b)
    {
        // Tocarlo: es de los recientes.
        let _ = std::fs::File::options().append(true).open(&destino);
        return Ok(m);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(ruta).arg(&destino);
    {
        use std::os::windows::process::CommandExt;
        // Sin ventana de consola.
        orden.creation_flags(0x0800_0000);
    }
    let t = std::time::Instant::now();
    let estado = orden.status().map_err(|e| e.to_string())?;
    tracing::info!(ms = t.elapsed().as_millis() as u64, codigo = ?estado.code(), ruta = %ruta.display(), "plano leido aparte");
    if !estado.success() {
        let motivo = std::fs::read_to_string(destino.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(destino.with_extension("error"));
        return Err(if motivo.is_empty() { "el plano no se pudo leer (puede estar dañado)".into() } else { motivo });
    }
    podar(&carpeta);
    let b = std::fs::read(&destino).map_err(|e| e.to_string())?;
    pixpin_cad::modelo::Modelo::de_bytes(&b).ok_or_else(|| "lo leido no se entiende".to_string())
}

fn textos(idioma: Idioma) -> pixpin_cad::ventana::TextosUi {
    match idioma {
        Idioma::Ingles => pixpin_cad::ventana::TextosUi {
            abriendo: "Opening the drawing…".into(),
            vacio: "The drawing has nothing to show".into(),
            error: "Could not open the drawing".into(),
            pista_acotar: "Click points to measure · Double-click or Enter: finish · Esc: exit".into(),
            todo: "Show whole drawing	F".into(),
            tema: "Light or dark background	B".into(),
            acotar: "Measure	M".into(),
            borrar_cotas: "Clear measurements	Del".into(),
            encima: "Always on top	T".into(),
            cerrar: "Close	Esc".into(),
            tres: "View in 3D	3".into(),
            marcos: "Frames to print	I".into(),
            imprimir: "Print…	Ctrl+P".into(),
            borrar_marcos: "Clear frames".into(),
            pista_marcos: "Drag to frame what is printed · Ctrl+P: print · Del: remove the last one".into(),
            anotar: "Annotate	A".into(),
            sin_dibujo_civil: "{n} Civil 3D objects were saved without their graphics: save with PROXYGRAPHICS = 1 or export to LandXML".into(),
        },
        _ => pixpin_cad::ventana::TextosUi::default(),
    }
}

/// Abre el plano en su ventana, en su propio hilo.
pub fn lanzar(idioma: Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let raiz = ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new().name("plano-cad".into()).spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let (r2, raiz2) = (ruta.clone(), raiz.clone());
        let _ = std::thread::Builder::new().name("plano-cad-leer".into()).spawn(move || {
            let _ = tx.send(cargar(&raiz2, &r2));
        });
        let titulo = ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        // El boton «3D»: el mismo plano en el visor de modelos.
        let (u3, r3) = (ubicacion.clone(), ruta.clone());
        let abrir_3d: Box<dyn Fn()> = Box::new(move || crate::modelo_bim::lanzar(idioma, u3.clone(), &r3));
        // Imprimir: el dialogo de Windows con la vista previa, cada marco una hoja.
        let (r4, raiz4) = (ruta.clone(), raiz.clone());
        let imprimir: Box<dyn Fn(isize, &pixpin_cad::modelo::Modelo, &[[f64; 4]], &[Vec<[f64; 2]>])> = Box::new(move |v, m, marcos, cotas| {
            let anotado = anotar::ParaImprimir::leer(&anotar::ruta_de_la_capa(&raiz4, &r4), m);
            if let Err(e) = imprimir::imprimir(v, m, marcos, cotas, anotado, &r4) {
                tracing::warn!(?e, "no se pudo imprimir el plano");
            }
        });
        // Anotar: el motor del lienzo sobre la capa del plano (anot-<uid>).
        let capa = anotar::ruta_de_la_capa(&raiz, &ruta);
        let anotador: Box<dyn FnOnce(&pixpin_cad::modelo::Modelo) -> Box<dyn pixpin_cad::anotado::Anotador>> =
            Box::new(move |_m| Box::new(anotar::MotorDelPlano::nuevo(capa)) as Box<dyn pixpin_cad::anotado::Anotador>);
        let acciones = pixpin_cad::ventana::Acciones { abrir_3d: Some(abrir_3d), imprimir: Some(imprimir), anotador: Some(anotador) };
        if let Err(e) = pixpin_cad::ventana::ver(&titulo, rx, textos(idioma), acciones) {
            tracing::warn!(%e, "no se pudo abrir el visor de planos");
            let _ = pixpin_shell::abrir(&ruta);
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el visor de planos");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_los_dwg_y_los_dxf() {
        assert!(se_abre("Planta.DWG") && se_abre("corte.dxf"));
        // Caso negativo: lo demas no.
        assert!(!se_abre("plano.pdf") && !se_abre("dwg"));
    }

    #[test]
    fn la_clave_cambia_si_cambia_el_fichero() {
        let d = std::env::temp_dir().join(format!("pixpin-cad-clave-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("a.dxf");
        std::fs::write(&f, "0\nEOF\n").unwrap();
        let k1 = clave(&f).unwrap();
        std::fs::write(&f, "0\nSECTION\n0\nEOF\n").unwrap();
        assert_ne!(clave(&f).unwrap(), k1);
        assert_eq!(clave(&d.join("no-esta.dwg")), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
