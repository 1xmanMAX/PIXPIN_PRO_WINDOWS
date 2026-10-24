//! Pruebas de las tablas del movil en el pin de hoja (12-oct-2026): una
//! `TABLA` del movil se abre, se edita y se guarda en SU fichero
//! (`tablas/<referencia>.json`, el que viaja), sin tocar el mensaje.

use super::*;
use pixpin_proyecto::almacen::{Ficha, Indice};
use pixpin_proyecto::cuaderno::{Cuaderno, Sello};
use pixpin_proyecto::tabla::ref_de;

fn raiz(nombre: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("pixpin-hoja-movil-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&r);
    r
}

/// Un proyecto con una tabla del movil ya sincronizada: su mensaje `TABLA`
/// y su fichero donde la deja la sincronizacion.
fn con_tabla_del_movil(raiz: &std::path::Path) -> (String, Mensaje, PathBuf) {
    let ficha = Ficha::nueva("obra", 0, "PC");
    let mut i = Indice::leer(raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(raiz).unwrap();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &ficha.id);
    let sello = Sello {
        cuando: 1_700_000_000_000,
        numero: 1,
        aparato: "MOVI".into(),
        proyecto: ficha.id.clone(),
    };
    let mut m = Mensaje::nota("", &sello);
    m.clase = Some(Clase::Otra("TABLA".into()));
    m.referencia = Some("t-1".into());
    m.nombre = "Metrados".into();
    pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();
    let archivo = carpeta.join("android").join("tablas").join("t-1.json");
    std::fs::create_dir_all(archivo.parent().unwrap()).unwrap();
    // Como la escribe el movil: lo que vale lo de por defecto no va, y lo
    // que el PC no conoce (`protegida`) tiene que seguir ahi.
    std::fs::write(&archivo, r#"{"nombre":"Metrados","celdas":{"A1":"1","B1":"x"},"estilos":{"B1":{"n":true}},"tocado":5,"protegida":true}"#).unwrap();
    (ficha.id, m, archivo)
}

#[test]
fn una_tabla_del_movil_se_abre_se_edita_y_se_guarda_en_su_fichero() {
    let raiz = raiz("editar");
    let (proyecto, m, archivo) = con_tabla_del_movil(&raiz);
    assert_eq!(que_abrir(&m), Some(Que::TablaDelMovil));
    let (mensaje, ruta) = tabla_del_movil(&raiz, &proyecto, &m).expect("se encuentra su fichero");
    assert_eq!(ruta, archivo);
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let mut ed = Editable::nuevo(
        u,
        proyecto.clone(),
        Origen::TablaDelMovil {
            mensaje,
            archivo: ruta,
        },
    );
    assert_eq!(ed.nombre(), "Metrados");
    ed.libro_para_ver(Regional::es_pe()).unwrap();
    let e = ed.editor.as_mut().unwrap();
    e.elegir(ref_de("A1").unwrap(), false);
    e.teclear('7');
    e.confirmar();
    ed.guardar().unwrap();
    let t = Tabla::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
    assert_eq!(
        t.celda(ref_de("A1").unwrap()),
        "7",
        "lo editado esta en el fichero que viaja"
    );
    assert_eq!(t.celda(ref_de("B1").unwrap()), "x");
    assert!(t.estilos["B1"].n, "el formato del movil sigue");
    assert_eq!(
        t.resto.get("protegida"),
        Some(&serde_json::Value::Bool(true)),
        "lo que el PC no conoce sigue"
    );
    assert!(t.tocado > 5, "con la hora de guardar");
    // El mensaje no se toca: la tabla no es su texto.
    let c = Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(&raiz, &proyecto)).unwrap();
    assert_eq!(c.mensajes.iter().find(|x| x.id == m.id).unwrap().texto, "");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn sin_su_fichero_o_sin_referencia_una_tabla_del_movil_no_se_abre() {
    let raiz = raiz("negativo");
    let (proyecto, m, archivo) = con_tabla_del_movil(&raiz);
    // Aun no llego su fichero: no se abre en blanco (se guardaria encima).
    std::fs::remove_file(&archivo).unwrap();
    assert!(tabla_del_movil(&raiz, &proyecto, &m).is_none());
    // Un fichero que no es una tabla, tampoco.
    std::fs::write(&archivo, "no es json").unwrap();
    assert!(tabla_del_movil(&raiz, &proyecto, &m).is_none());
    assert_eq!(
        std::fs::read_to_string(&archivo).unwrap(),
        "no es json",
        "el fichero roto sigue tal cual"
    );
    // Sin referencia no hay tabla que buscar; y una nota no es una tabla.
    let sin = Mensaje {
        referencia: None,
        ..m.clone()
    };
    assert_eq!(que_abrir(&sin), None);
    let nota = Mensaje {
        clase: Some(Clase::Nota),
        ..m
    };
    assert_eq!(que_abrir(&nota), None);
    let _ = std::fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// Las hojas de un Excel que el movil editaba antes del 13-oct

/// Un proyecto con un libro de una hoja adjunto (lo que hay en el `.xlsx`:
/// «Original» en A1) y, como lo dejaba el movil antes del 13-oct, sus hojas
/// editadas solo en `tablas/libro-<mensaje>-<hoja>.json`.
fn con_libro_y_hojas_viejas(
    raiz: &std::path::Path,
    viejas: &[&str],
) -> (String, Mensaje, PathBuf, Vec<PathBuf>) {
    use pixpin_proyecto::tabla_xlsx::{HojaParaVer, a_xlsx};
    let ficha = Ficha::nueva("obra", 0, "PC");
    let mut i = Indice::leer(raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(raiz).unwrap();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(&carpeta).unwrap();
    let mut t = Tabla::default();
    t.poner(ref_de("A1").unwrap(), "Original");
    let bytes = a_xlsx(
        &[HojaParaVer {
            nombre: "Hoja1",
            tabla: &t,
            valores: None,
        }],
        0,
    )
    .unwrap();
    let ruta = carpeta.join("Cuentas.xlsx");
    std::fs::write(&ruta, &bytes).unwrap();
    let sello = Sello {
        cuando: 1_700_000_000_000,
        numero: 1,
        aparato: "MOVI".into(),
        proyecto: ficha.id.clone(),
    };
    let m = Mensaje::adjunto(
        Clase::Archivo,
        "Cuentas.xlsx",
        "Cuentas.xlsx",
        bytes.len() as i64,
        &sello,
    );
    pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();
    let tablas = carpeta.join("android").join("tablas");
    std::fs::create_dir_all(&tablas).unwrap();
    let archivos: Vec<PathBuf> = viejas
        .iter()
        .enumerate()
        .map(|(i, texto)| {
            let a = tablas.join(format!("libro-{}-{i}.json", m.id));
            std::fs::write(&a, texto).unwrap();
            a
        })
        .collect();
    (ficha.id, m, ruta, archivos)
}

#[test]
fn las_hojas_viejas_del_movil_se_editan_y_al_guardar_pasan_a_mensajes() {
    let raiz = raiz("viejas");
    // La segunda, con un estilo del formato de antes (una palabra) y un
    // campo que el PC no conoce: se abre igual y no se pierde.
    let hoja0 = r##"{"nombre":"Cuentas · Gastos","celdas":{"A1":"Editado en el movil","B1":"5"},"tocado":1700000500000,"protegida":true}"##;
    let hoja1 = r##"{"nombre":"Cuentas · Resumen","celdas":{"A1":"Total"},"estilos":{"A1":"negrita"},"futuro":{"x":1}}"##;
    let (proyecto, m, ruta, archivos) = con_libro_y_hojas_viejas(&raiz, &[hoja0, hoja1]);
    let antes: Vec<Vec<u8>> = archivos.iter().map(|a| std::fs::read(a).unwrap()).collect();
    assert_eq!(
        pixpin_proyecto::vista::tablas_viejas_del_libro(&raiz, &proyecto, &m.id),
        archivos,
        "se encuentran donde las deja la sincronizacion"
    );

    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let mut ed = Editable::nuevo(
        u,
        proyecto.clone(),
        Origen::Libro {
            mensaje: m.clone(),
            ruta,
            tablas: Vec::new(),
        },
    );
    ed.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(
        !ed.vista_original,
        "lo editado en el movil no se esconde tras el original"
    );
    let e = ed.editor.as_mut().unwrap();
    assert_eq!(
        e.hojas.len(),
        2,
        "las del movil, no el libro leido otra vez"
    );
    assert_eq!(
        e.hojas[0].tabla.celda(ref_de("A1").unwrap()),
        "Editado en el movil"
    );
    assert!(
        e.hojas[1].tabla.estilos["A1"].n,
        "la negrita vieja se migra"
    );
    e.elegir(ref_de("B1").unwrap(), false);
    e.teclear('8');
    e.confirmar();
    ed.guardar().unwrap();

    // Ahora son mensajes `tabla` con la referencia del libro: el formato
    // unico, el que el movil lee primero.
    let c = Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(&raiz, &proyecto)).unwrap();
    let hechas = libro::tablas_hechas(&c.mensajes, &m);
    assert_eq!(hechas.len(), 2);
    let gastos = &c.mensajes[hechas[0]];
    assert_eq!(
        gastos.referencia.as_deref(),
        Some(format!("libro-{}-0", m.id).as_str())
    );
    let t0 = Tabla::leer(&gastos.texto).unwrap();
    assert_eq!(t0.celda(ref_de("B1").unwrap()), "8");
    assert_eq!(t0.resto["protegida"], true, "lo que el PC no conoce sigue");
    assert!(libro::editada(gastos));
    let t1: serde_json::Value = serde_json::from_str(&c.mensajes[hechas[1]].texto).unwrap();
    assert_eq!(
        t1["estilos"]["A1"],
        serde_json::json!({"n": true}),
        "el estilo sale en el formato de ahora"
    );
    assert_eq!(t1["futuro"]["x"], 1);
    // Los ficheros viejos no se tocan (el movil los conserva tambien).
    let despues: Vec<Vec<u8>> = archivos.iter().map(|a| std::fs::read(a).unwrap()).collect();
    assert_eq!(antes, despues);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn una_hoja_vieja_rota_no_se_pierde_y_se_lee_el_libro() {
    let raiz = raiz("vieja-rota");
    let (proyecto, m, ruta, archivos) =
        con_libro_y_hojas_viejas(&raiz, &["{ esto no es una tabla"]);
    assert!(
        tablas_viejas_del_libro(&raiz, &proyecto, &m).is_none(),
        "una hoja que no se entiende no se usa"
    );
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let mut ed = Editable::nuevo(
        u,
        proyecto.clone(),
        Origen::Libro {
            mensaje: m.clone(),
            ruta,
            tablas: Vec::new(),
        },
    );
    ed.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(
        ed.vista_original,
        "sin hojas viejas que valgan se ve el original"
    );
    ed.montar_editor().unwrap();
    let e = ed.editor.as_mut().unwrap();
    assert_eq!(
        e.hojas[0].tabla.celda(ref_de("A1").unwrap()),
        "Original",
        "se lee el libro, como si no hubiera nada"
    );
    e.elegir(ref_de("A2").unwrap(), false);
    e.teclear('1');
    e.confirmar();
    ed.guardar().unwrap();
    assert_eq!(
        std::fs::read_to_string(&archivos[0]).unwrap(),
        "{ esto no es una tabla",
        "el fichero roto sigue tal cual"
    );
    // Sin ficheros viejos, nada que migrar; y un libro sin id no busca.
    assert!(pixpin_proyecto::vista::tablas_viejas_del_libro(&raiz, &proyecto, "otro").is_empty());
    assert!(pixpin_proyecto::vista::tablas_viejas_del_libro(&raiz, &proyecto, " ").is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}
