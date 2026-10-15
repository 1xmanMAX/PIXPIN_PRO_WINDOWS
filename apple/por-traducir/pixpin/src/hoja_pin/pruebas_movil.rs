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
    let sello = Sello { cuando: 1_700_000_000_000, numero: 1, aparato: "MOVI".into(), proyecto: ficha.id.clone() };
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
    let mut ed = Editable::nuevo(u, proyecto.clone(), Origen::TablaDelMovil { mensaje, archivo: ruta });
    assert_eq!(ed.nombre(), "Metrados");
    ed.libro_para_ver(Regional::es_pe()).unwrap();
    let e = ed.editor.as_mut().unwrap();
    e.elegir(ref_de("A1").unwrap(), false);
    e.teclear('7');
    e.confirmar();
    ed.guardar().unwrap();
    let t = Tabla::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
    assert_eq!(t.celda(ref_de("A1").unwrap()), "7", "lo editado esta en el fichero que viaja");
    assert_eq!(t.celda(ref_de("B1").unwrap()), "x");
    assert!(t.estilos["B1"].n, "el formato del movil sigue");
    assert_eq!(t.resto.get("protegida"), Some(&serde_json::Value::Bool(true)), "lo que el PC no conoce sigue");
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
    // Sin referencia no hay tabla que buscar; y una nota no es una tabla.
    let sin = Mensaje { referencia: None, ..m.clone() };
    assert_eq!(que_abrir(&sin), None);
    let nota = Mensaje { clase: Some(Clase::Nota), ..m };
    assert_eq!(que_abrir(&nota), None);
    let _ = std::fs::remove_dir_all(&raiz);
}
