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
pub(crate) mod anotar;
/// Exportar el plano: HTML por capas, PDF, PNG y DXF (13-oct-2026).
pub(crate) mod exportar;
/// Los textos del plano con su sitio, para buscar en ellos (14-oct-2026).
pub(crate) mod textos;

/// Donde mirar al abrir un plano (`[x, y, ancho]` del dibujo): el
/// asistente llevando a un texto encontrado (`abrir` con `zona`). Como la
/// pagina del lector de PDF, se toma al abrir.
static VISTA_AL_ABRIR: std::sync::Mutex<Vec<(PathBuf, [f64; 3])>> = std::sync::Mutex::new(Vec::new());

/// **Que la proxima vez que se abra `plano` mire ahi.**
pub fn abrir_en(plano: &Path, vista: [f64; 3]) {
    let mut v = VISTA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    v.retain(|(r, _)| r != plano);
    v.push((plano.to_path_buf(), vista));
}

fn tomar_vista_al_abrir(plano: &Path) -> Option<[f64; 3]> {
    let mut v = VISTA_AL_ABRIR.lock().unwrap_or_else(|e| e.into_inner());
    let i = v.iter().position(|(r, _)| r == plano)?;
    Some(v.remove(i).1)
}

/// El plano leido (de la cache o en el proceso aparte), para quien no es
/// el visor: el asistente anotando encima.
pub(crate) fn modelo(raiz: &Path, plano: &Path) -> Result<pixpin_cad::modelo::Modelo, String> {
    cargar(raiz, plano)
}

/// La orden con que PixPin se llama a si misma para exportar un plano.
pub const ORDEN_EXPORTAR: &str = exportar::ORDEN;

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
    if args.get(1).is_some_and(|a| a == ORDEN_EXPORTAR) {
        return Some(exportar::en_este_proceso(&args[2..]));
    }
    if args.get(1).is_some_and(|a| a == textos::ORDEN) {
        return Some(textos::en_este_proceso(&args[2..]));
    }
    if args.get(1).is_none_or(|a| a != ORDEN) {
        return None;
    }
    let (Some(entrada), Some(salida)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    // La carpeta de letras del usuario (el boton «Aa»), si la hay: manda
    // sobre las de Autodesk, Windows y Hershey.
    let letras = args.get(4).map(PathBuf::from);
    Some(match pixpin_cad::letras::convertir_con_letras(&entrada, letras) {
        Ok(modelo) => {
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

/// Cambia cuando cambia lo que se guarda del plano leido: la `l1` trae las
/// letras que faltan (lo leido antes no las tenia y se vuelve a leer).
const REVISION_DE_LETRAS: &str = "l1";

/// El nombre del plano leido en la cache: su clave, la revision y, si el
/// usuario puso letras, su huella (`huellaDeLetras` de Android): con una
/// letra nueva el plano se vuelve a leer con ella.
fn nombre_en_cache(clave: &str, huella: &str) -> String {
    if huella.is_empty() {
        format!("{clave}-{REVISION_DE_LETRAS}.pxcad")
    } else {
        format!("{clave}-{REVISION_DE_LETRAS}-{huella}.pxcad")
    }
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
    let letras = pixpin_cad::letras::carpeta_en(raiz);
    let destino = carpeta.join(nombre_en_cache(&clave, &pixpin_cad::letras::huella(&letras)));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(m) = pixpin_cad::modelo::Modelo::de_bytes(&b)
    {
        // Tocarlo: es de los recientes.
        let _ = std::fs::File::options().append(true).open(&destino);
        return Ok(m);
    }
    // En las pruebas el ejecutable es el de las pruebas, que no sabe la
    // orden: se lee aqui mismo.
    if cfg!(test) {
        return pixpin_cad::letras::convertir_con_letras(ruta, Some(letras));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(ruta).arg(&destino).arg(&letras);
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
            exportar: "Export (HTML, PDF, PNG, DXF…)…	Ctrl+E".into(),
            borrar_marcos: "Clear frames".into(),
            pista_marcos: "Drag to frame what is printed · Ctrl+P: print · Del: remove the last one".into(),
            anotar: "Annotate	A".into(),
            sin_dibujo_civil: "{n} Civil 3D objects were saved without their graphics: save with PROXYGRAPHICS = 1 or export to LandXML".into(),
            letras: "Drawing fonts…	L".into(),
            letras_faltan: "This drawing asks for fonts that are not here; its texts use similar ones and may not fit their boxes:".into(),
            letras_no_falta: "No font that the drawing names is missing.".into(),
            letras_donde: concat!(
                r"SHX fonts are usually in C:\Program Files\Autodesk\AutoCAD 20xx\Fonts or next to the drawing you were sent;",
                "\n",
                r"TrueType ones, in C:\Windows\Fonts. The ones you add here win over AutoCAD's, Windows' and PixPin's."
            )
            .into(),
            letras_anadir: "Add fonts (SHX, TTF, OTF, TTC)…".into(),
            letras_quitar: "Remove mine ({n})".into(),
            faltan_letras: "{n} fonts of the drawing are missing: click Aa above to add them".into(),
            releyendo: "Reading the drawing with your fonts…".into(),
        },
        _ => pixpin_cad::ventana::TextosUi::default(),
    }
}

/// Lee el plano en otro hilo; llega por el canal.
fn leer_aparte(raiz: &Path, ruta: &Path) -> std::sync::mpsc::Receiver<Result<pixpin_cad::modelo::Modelo, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (r2, raiz2) = (ruta.to_path_buf(), raiz.to_path_buf());
    let _ = std::thread::Builder::new().name("plano-cad-leer".into()).spawn(move || {
        let _ = tx.send(cargar(&raiz2, &r2));
    });
    rx
}

/// Abre el plano en su ventana, en su propio hilo.
pub fn lanzar(idioma: Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let raiz = ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new().name("plano-cad".into()).spawn(move || {
        // El dialogo «Abrir» del boton «Aa» es COM.
        let _com = pixpin_shell::ComDelHilo::iniciar();
        // Las letras: fuera lo que llego y no lo es, y que viajen las que el
        // usuario ya tenia. Antes de leer, para leer con lo que queda.
        crate::letras_que_viajan::al_abrir(&raiz);
        let rx = leer_aparte(&raiz, &ruta);
        let titulo = ruta.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        // El boton «3D»: el mismo plano en el visor de modelos.
        let (u3, r3) = (ubicacion.clone(), ruta.clone());
        let abrir_3d: Box<dyn Fn()> = Box::new(move || crate::modelo_bim::lanzar(idioma, u3.clone(), &r3));
        // Imprimir: el dialogo de Windows con la vista previa, cada marco una hoja.
        let (r4, raiz4) = (ruta.clone(), raiz.clone());
        let imprimir: Box<dyn Fn(isize, &pixpin_cad::modelo::Modelo, &[[f64; 4]], &[Vec<[f64; 2]>])> = Box::new(move |v, m, marcos, cotas| {
            let anotado = anotar::ParaImprimir::leer(&anotar::rutas_de_la_capa(&raiz4, &r4), m);
            if let Err(e) = imprimir::imprimir(v, m, marcos, cotas, anotado, &r4) {
                tracing::warn!(?e, "no se pudo imprimir el plano");
            }
        });
        // Anotar: el motor del lienzo sobre la capa del plano (anot-<uid>).
        let capa = anotar::rutas_de_la_capa(&raiz, &ruta);
        let anotador: Box<dyn FnOnce(&pixpin_cad::modelo::Modelo) -> Box<dyn pixpin_cad::anotado::Anotador>> =
            Box::new(move |m| Box::new(anotar::MotorDelPlano::nuevo(capa, m)) as Box<dyn pixpin_cad::anotado::Anotador>);
        // El boton «Aa»: las letras del usuario, en `<raiz>/fuentes-shx`, que
        // viajan con sus otros aparatos (`letras_que_viajan`).
        let carpeta = pixpin_cad::letras::carpeta_en(&raiz);
        let (c1, c4, r5, raiz5) = (carpeta.clone(), carpeta, ruta.clone(), raiz.clone());
        let (raiz_a, raiz_q) = (raiz.clone(), raiz.clone());
        let letras = pixpin_cad::ventana::Letras {
            mias: Box::new(move || pixpin_cad::letras::letras_en(&c1).len()),
            anadir: Box::new(move |v| {
                let elegidas = pixpin_shell::elegir::pedir_ficheros(windows::Win32::Foundation::HWND(v as *mut _));
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                crate::letras_que_viajan::anadir(&raiz_a, &crate::letras_que_viajan::aparato(&raiz_a), ahora, &elegidas)
            }),
            quitar: Box::new(move || crate::letras_que_viajan::quitar_todas(&raiz_q, pixpin_shell::entorno::ahora_utc_ms())),
            releer: Box::new(move || leer_aparte(&raiz5, &r5)),
            huella: Box::new(move || pixpin_cad::letras::huella(&c4)),
        };
        // Exportar: el componente comun con los formatos del plano.
        let (r6, raiz6, u6) = (ruta.clone(), raiz.clone(), ubicacion.clone());
        let exportar: Box<dyn Fn(isize, &pixpin_cad::modelo::Modelo, &[[f64; 4]], &[Vec<[f64; 2]>])> = Box::new(move |_, m, marcos, cotas| {
            let p = exportar::Plano::nuevo(&raiz6, &r6, m, cotas, marcos);
            crate::compartir::ventana::abrir(idioma, u6.clone(), crate::compartir::Cosa::Visor(exportar::exportable(p, idioma)));
        });
        let acciones = pixpin_cad::ventana::Acciones {
            abrir_3d: Some(abrir_3d),
            imprimir: Some(imprimir),
            anotador: Some(anotador),
            letras: Some(letras),
            exportar: Some(exportar),
            vista_inicial: tomar_vista_al_abrir(&ruta),
        };
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
    fn con_letras_nuevas_el_plano_se_vuelve_a_leer() {
        // Sin letras del usuario, el nombre no lleva huella.
        assert_eq!(nombre_en_cache("abc", ""), "abc-l1.pxcad");
        let d = std::env::temp_dir().join(format!("pixpin-cad-huella-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let vacia = pixpin_cad::letras::huella(&d);
        assert_eq!(nombre_en_cache("abc", &vacia), "abc-l1.pxcad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("romans.shx"), b"x").unwrap();
        let con = nombre_en_cache("abc", &pixpin_cad::letras::huella(&d));
        assert_ne!(con, "abc-l1.pxcad");
        assert!(con.starts_with("abc-l1-") && con.ends_with(".pxcad"));
        let _ = std::fs::remove_dir_all(&d);
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
